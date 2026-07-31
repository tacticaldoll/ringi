//! The reconcile loop — the bet.
//!
//! One consumer loop reconciles a desired set of steps to done by composing three bricks
//! through their public APIs, adding no lifecycle/convergence/idempotency of its own:
//!
//! - **suunta** plans the residual (which steps remain) and decides convergence;
//! - **pacta** (over the `pacta-memory` reference backend) owns each step's durable
//!   claim -> execute -> settle, with `release(reclaimable_at)` for backoff'd retry;
//! - **shaahid** witnesses each step attempt so its side effect happens exactly once, even
//!   when a claim is reclaimed after the work already succeeded.
//!
//! Everything ringi adds is the loop and the thin `seam` adapters (identity mapping,
//! findings translation). Steps are stubs; there are no agents yet — the bet is about
//! composition, not agents.

use std::collections::{HashMap, HashSet};

use pacta::{Registry, Timestamp};
use pacta_memory::MemoryRegistry;
use shaahid::{Attestation, Deed, witness};
use suunta::{
    Bearing, Correction, Fix, Reversibility, Satisfaction, SatisfactionFinding, Sigil, Sounding,
    plan_residual,
};

const LEASE_MILLIS: u64 = 1_000;
const BACKOFF_MILLIS: u64 = 500;
/// Advance per cycle past both the lease and the backoff, so a released or lapsed step is
/// reclaimable on the next cycle.
const TICK_MILLIS: u64 = 1_500;
/// Consumer-held safety bound (this is Layer-3 termination — the loop's, not the core's).
const MAX_CYCLES: usize = 32;

/// How a stub step behaves, to exercise each brick's edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepMode {
    /// Succeeds on the first attempt.
    Normal,
    /// Fails its first attempt (exercises pacta `release` + retry), then succeeds.
    FailsFirst,
    /// Succeeds but its settlement is "lost" once, so the lease lapses and it is reclaimed;
    /// shaahid must no-op the re-execution (exercises exactly-once).
    LapsesAfterSuccess,
}

/// A stub unit of work. In a real run this carries the Agent-CLI task; here it is minimal.
#[derive(Debug, Clone)]
pub struct StepSpec {
    /// Stable identity of the step (becomes its `Sigil`, `Seal`, and docket).
    pub id: String,
    /// Behavior archetype.
    pub mode: StepMode,
}

impl StepSpec {
    /// A step of a given mode.
    #[must_use]
    pub fn new(id: impl Into<String>, mode: StepMode) -> Self {
        Self {
            id: id.into(),
            mode,
        }
    }
}

/// The observable outcome of a reconcile run, for asserting the bet.
#[derive(Debug)]
pub struct Report {
    /// Cycles the loop ran.
    pub cycles: usize,
    /// Whether suunta reported the run converged (vs. hitting the cycle bound).
    pub converged: bool,
    /// Per-step count of how many times its side effect actually executed.
    pub executions: HashMap<String, u32>,
    /// Steps that failed once and were retried via deferred reclaim.
    pub retried: HashSet<String>,
    /// Steps whose re-execution after a lapse was suppressed by shaahid.
    pub deduplicated: HashSet<String>,
}

/// Thin seam adapters — mapping only. Any logic here beyond translation is the monolith
/// returning.
mod seam {
    use super::{Correction, Deed, Sigil, StepSpec};
    use pacta::Pact;
    use shaahid::Fingerprint;
    use uuid::Uuid;

    /// Deterministic identity bridge: a step's `Sigil` (a string) -> the `Pact`'s `Uuid`.
    /// The `Sigil` string also rides in the clause, so nothing is lost.
    fn pact_id(sigil: &Sigil) -> Uuid {
        Uuid::new_v5(&Uuid::NAMESPACE_OID, sigil.as_str().as_bytes())
    }

    /// A step becomes a pact on its own docket (so it is individually claimable).
    pub fn step_to_pact(correction: &Correction<StepSpec>) -> Pact {
        let sigil = correction.sigil();
        Pact::new(
            pact_id(sigil),
            sigil.as_str().to_string(),
            "step".to_string(),
            sigil.as_str().as_bytes().to_vec(),
        )
    }

    /// A step attempt becomes a `Deed`: `Seal` is the step identity (Sigil == Seal), the
    /// fingerprint is the content identity of the step spec.
    pub fn deed_for_step(correction: &Correction<StepSpec>) -> Deed<String> {
        let sigil = correction.sigil();
        Deed::new(
            sigil.as_str().to_string(),
            Fingerprint::new(correction.body().id.as_bytes().to_vec()),
        )
    }
}

/// Reconcile `steps` to done, composing the family. Returns a [`Report`] to assert on.
///
/// # Panics
///
/// Panics only if the reference registry returns an error, which it cannot for the calls
/// made here (they are always on a valid current holder or a fresh claim).
#[must_use]
pub fn run(steps: Vec<StepSpec>) -> Report {
    // The desired set, as a suunta Bearing (each step a Correction identified by a Sigil).
    let targets: Vec<Correction<StepSpec>> = steps
        .iter()
        .map(|s| Correction::new(Sigil::new(&s.id), Reversibility::Reversible, s.clone()))
        .collect();

    // Seed the reference backend with one pact per step (ingress is the backend's; the
    // reference seeds at construction).
    let registry = MemoryRegistry::seeded(
        targets.iter().map(seam::step_to_pact).collect(),
        LEASE_MILLIS,
    );

    // Consumer state — none of it a lifecycle/convergence/idempotency engine.
    let mut ledger: Vec<Deed<String>> = Vec::new(); // shaahid: steps whose work succeeded
    let mut done: HashSet<String> = HashSet::new(); // domain satisfaction
    let mut executions: HashMap<String, u32> = HashMap::new();
    let mut failed_once: HashSet<String> = HashSet::new();
    let mut lapsed_once: HashSet<String> = HashSet::new();
    let mut retried: HashSet<String> = HashSet::new();
    let mut deduplicated: HashSet<String> = HashSet::new();

    let mut now = 0u64;
    let mut cycles = 0usize;
    let mut converged = false;

    while cycles < MAX_CYCLES {
        cycles += 1;

        // Sounding: certify per-target satisfaction from the domain's world. (No coverage:
        // this synchronous loop claims and settles within a cycle, so there is no
        // cross-cycle in-flight to report.)
        let findings = targets
            .iter()
            .map(|c| SatisfactionFinding {
                target: c.sigil().clone(),
                satisfaction: if done.contains(c.sigil().as_str()) {
                    Satisfaction::Satisfied
                } else {
                    Satisfaction::Unsatisfied
                },
            })
            .collect();
        let sounding = Sounding::new(Fix::new(findings), Vec::new());

        // suunta decides what remains and whether we are done.
        let residual = plan_residual(Bearing::new(targets.clone()), &sounding);
        if residual.is_converged() {
            converged = true;
            break;
        }

        for correction in residual.course.corrections() {
            let sigil = correction.sigil().as_str().to_string();
            let now_ts = Timestamp::from_millis(now);

            // pacta: claim this step from its own docket. `None` = released/lapsed and not
            // yet reclaimable this cycle.
            let Some(claim) = registry
                .claim(&[sigil.as_str()], now_ts)
                .expect("memory registry claim never errors")
            else {
                continue;
            };

            // shaahid: witness the attempt for exactly-once.
            let deed = seam::deed_for_step(correction);
            let outcome = witness(&ledger, deed.clone());
            if !outcome.contradictions.is_empty() {
                // Quarantine: surfaced, never silently retried. (No contradictions arise
                // for these stubs; wired for correctness.)
                continue;
            }

            match outcome.attestation {
                // Already performed (its deed is on the ledger): a reclaim after a lost
                // settlement. Do not re-execute — just settle. This is exactly-once.
                Attestation::Attach(_) => {
                    deduplicated.insert(sigil.clone());
                    done.insert(sigil.clone());
                    registry.fulfill(&claim.retainer).expect("fulfill");
                }
                Attestation::Create => match correction.body().mode {
                    // Fail the first attempt: release with a backoff instant, record nothing.
                    StepMode::FailsFirst if !failed_once.contains(&sigil) => {
                        failed_once.insert(sigil.clone());
                        retried.insert(sigil.clone());
                        registry
                            .release(
                                &claim.retainer,
                                Timestamp::from_millis(now + BACKOFF_MILLIS),
                            )
                            .expect("release");
                    }
                    // Succeed but "lose" the settlement once: record the deed and execute,
                    // but do not fulfill or mark done, so the lease lapses and reclaims.
                    StepMode::LapsesAfterSuccess if !lapsed_once.contains(&sigil) => {
                        lapsed_once.insert(sigil.clone());
                        *executions.entry(sigil.clone()).or_insert(0) += 1;
                        ledger.push(deed);
                    }
                    // Normal, or a retry, or any first-time success: perform once, record,
                    // settle, and mark done.
                    _ => {
                        *executions.entry(sigil.clone()).or_insert(0) += 1;
                        ledger.push(deed);
                        done.insert(sigil.clone());
                        registry.fulfill(&claim.retainer).expect("fulfill");
                    }
                },
            }
        }

        now += TICK_MILLIS;
    }

    Report {
        cycles,
        converged,
        executions,
        retried,
        deduplicated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_family_composes_into_a_convergent_exactly_once_durable_reconcile() {
        let report = run(vec![
            StepSpec::new("step:normal", StepMode::Normal),
            StepSpec::new("step:fails-first", StepMode::FailsFirst),
            StepSpec::new("step:lapses", StepMode::LapsesAfterSuccess),
        ]);

        // suunta drove it to convergence, not the cycle bound.
        assert!(
            report.converged,
            "the run must converge (suunta): {report:?}"
        );

        // Every step's side effect happened exactly once — across a retry and a reclaim.
        for id in ["step:normal", "step:fails-first", "step:lapses"] {
            assert_eq!(
                report.executions.get(id).copied(),
                Some(1),
                "{id} must execute exactly once: {report:?}"
            );
        }

        // pacta release drove the retry; shaahid suppressed the post-lapse re-execution.
        assert!(
            report.retried.contains("step:fails-first"),
            "the failing step must retry via deferred reclaim: {report:?}"
        );
        assert!(
            report.deduplicated.contains("step:lapses"),
            "the reclaimed-after-success step must be deduplicated by shaahid: {report:?}"
        );
    }
}
