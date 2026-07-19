//! The reconcile loop — the bet.
//!
//! One consumer loop reconciles a desired set of steps to done by composing three bricks
//! through their public APIs, adding no lifecycle/convergence/idempotency of its own:
//!
//! - **suunta** plans the residual (which steps remain) and decides convergence;
//! - **pacta** (over any `Registry` backend — the reference `pacta-memory` or the durable
//!   `SqliteRegistry`) owns each step's claim -> execute -> settle, with
//!   `release(reclaimable_at)` for backoff'd retry;
//! - **shaahid** witnesses each step attempt so its side effect happens exactly once, even
//!   when a claim is reclaimed after the work already succeeded.
//!
//! Everything ringi adds is the loop and the thin `seam` adapters (identity mapping,
//! findings translation). A step's actual work is delegated to a [`StepRunner`]: the loop acts
//! only on the outcome it returns, never deciding success itself. The production runner
//! ([`AgentStepRunner`]) runs a Builder agent; a scripted runner drives the composition bet.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Duration;

use pacta::{Pact, Registry, Timestamp};
use pacta_memory::MemoryRegistry;
use shaahid::{Attestation, Deed, witness};
use suunta::{
    Bearing, Correction, Fix, Reversibility, Satisfaction, SatisfactionFinding, Sigil, Sounding,
    plan_residual,
};

use crate::agent::{AgentAdapter, AgentRequest, AgentRole};

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

/// The result of running a step's work. The loop settles a `Succeeded` step and retries a
/// `Failed` one; it never decides which itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepOutcome {
    /// The step's work completed.
    Succeeded,
    /// The step's work did not complete; the loop retries it via deferred reclaim.
    Failed,
}

/// The seam by which the loop performs a step's actual work. This is ringi's own blood — the
/// bricks own lifecycle, convergence, and idempotency; the runner owns only "do the work, did
/// it succeed."
pub trait StepRunner {
    /// Perform `step`'s work and report whether it succeeded.
    fn run(&self, step: &StepSpec) -> StepOutcome;
}

/// Runs a step's work by invoking a Builder agent through the agent seam. Depends only on the
/// [`AgentAdapter`] trait, so any adapter can back it.
#[derive(Debug, Clone)]
pub struct AgentStepRunner<A> {
    adapter: A,
    workspace: PathBuf,
    timeout: Duration,
}

impl<A: AgentAdapter> AgentStepRunner<A> {
    /// A Builder runner over `adapter`, running each step in `workspace` bounded by `timeout`.
    pub fn new(adapter: A, workspace: impl Into<PathBuf>, timeout: Duration) -> Self {
        Self {
            adapter,
            workspace: workspace.into(),
            timeout,
        }
    }
}

impl<A: AgentAdapter> StepRunner for AgentStepRunner<A> {
    fn run(&self, step: &StepSpec) -> StepOutcome {
        let request = AgentRequest {
            role: AgentRole::Builder,
            prompt: format!("Perform step: {}", step.id),
            workspace: self.workspace.clone(),
            timeout: self.timeout,
            env: HashMap::new(),
        };
        // A clean exit is success; a non-zero exit, spawn failure, or timeout is a failure the
        // loop retries. The runner computes no backoff — that is pacta's, loop-driven.
        match self.adapter.run(request) {
            Ok(response) if response.exit_code == Some(0) => StepOutcome::Succeeded,
            _ => StepOutcome::Failed,
        }
    }
}

/// The scripted runner that drives the composition bet: a `FailsFirst` step fails its first
/// attempt then succeeds; every other mode succeeds. Interior-mutable so the loop can hold it
/// immutably while it tracks which steps have already failed once.
struct ScriptedRunner {
    failing_once: RefCell<HashSet<String>>,
}

impl ScriptedRunner {
    fn from_modes(steps: &[StepSpec]) -> Self {
        Self {
            failing_once: RefCell::new(
                steps
                    .iter()
                    .filter(|s| s.mode == StepMode::FailsFirst)
                    .map(|s| s.id.clone())
                    .collect(),
            ),
        }
    }
}

impl StepRunner for ScriptedRunner {
    fn run(&self, step: &StepSpec) -> StepOutcome {
        if self.failing_once.borrow_mut().remove(&step.id) {
            StepOutcome::Failed
        } else {
            StepOutcome::Succeeded
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

/// Reconcile `steps` to done over the reference in-memory backend, with the steps' modes
/// scripted. See [`run_with`].
#[must_use]
pub fn run(steps: Vec<StepSpec>) -> Report {
    run_with(steps, MemoryRegistry::seeded)
}

/// Reconcile `steps` to done over any backend built by `make`, with the steps' modes scripted.
///
/// `make(pacts, lease_millis)` returns a seeded [`Registry`] — the same constructor shape
/// `pacta-conformance` uses — so the loop is backend-agnostic: the reference backend and a
/// durable `SqliteRegistry` run the identical composition. This is the scripted composition
/// bet: step outcomes come from a mode-derived runner and `LapsesAfterSuccess` injects a lost
/// settlement. For a real injected runner (e.g. an agent), see [`run_with_runner`]. Returns a
/// [`Report`] to assert on.
///
/// # Panics
///
/// Panics only if the registry returns an error, which it does not for the calls made here
/// (always a valid current holder or a fresh claim).
#[must_use]
pub fn run_with<R, F>(steps: Vec<StepSpec>, make: F) -> Report
where
    R: Registry,
    R::Error: std::fmt::Debug,
    F: FnOnce(Vec<Pact>, u64) -> R,
{
    let runner = ScriptedRunner::from_modes(&steps);
    // The lapse is a durability fault (a lost settlement), not a runner outcome, so it stays
    // loop-level: these steps drop their first settlement and are reclaimed.
    let lapse_once: HashSet<String> = steps
        .iter()
        .filter(|s| s.mode == StepMode::LapsesAfterSuccess)
        .map(|s| s.id.clone())
        .collect();
    run_core(steps, make, &runner, lapse_once)
}

/// Reconcile `steps` to done over any backend built by `make`, delegating each step's work to
/// `runner` — the production entry point (no fault injection). Returns a [`Report`].
///
/// # Panics
///
/// Panics only if the registry returns an error, which it does not for the calls made here.
#[must_use]
pub fn run_with_runner<R, F>(steps: Vec<StepSpec>, make: F, runner: &dyn StepRunner) -> Report
where
    R: Registry,
    R::Error: std::fmt::Debug,
    F: FnOnce(Vec<Pact>, u64) -> R,
{
    run_core(steps, make, runner, HashSet::new())
}

/// The single loop body, parameterized over the registry backend, the step runner, and the
/// (test-only) set of steps that lapse their first settlement.
fn run_core<R, F>(
    steps: Vec<StepSpec>,
    make: F,
    runner: &dyn StepRunner,
    mut lapse_once: HashSet<String>,
) -> Report
where
    R: Registry,
    R::Error: std::fmt::Debug,
    F: FnOnce(Vec<Pact>, u64) -> R,
{
    // The desired set, as a suunta Bearing (each step a Correction identified by a Sigil).
    let targets: Vec<Correction<StepSpec>> = steps
        .iter()
        .map(|s| Correction::new(Sigil::new(&s.id), Reversibility::Reversible, s.clone()))
        .collect();

    // Build one pact per step and seed the backend (ingress is the backend's).
    let registry = make(
        targets.iter().map(seam::step_to_pact).collect(),
        LEASE_MILLIS,
    );

    // Consumer state — none of it a lifecycle/convergence/idempotency engine.
    let mut ledger: Vec<Deed<String>> = Vec::new(); // shaahid: steps whose work succeeded
    let mut done: HashSet<String> = HashSet::new(); // domain satisfaction
    let mut executions: HashMap<String, u32> = HashMap::new();
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
                // Not yet performed: run the step's work and act only on its outcome.
                Attestation::Create => match runner.run(correction.body()) {
                    // Failed: release with a backoff instant, record nothing. The step is
                    // withheld until the instant, then reclaimed and retried.
                    StepOutcome::Failed => {
                        retried.insert(sigil.clone());
                        registry
                            .release(
                                &claim.retainer,
                                Timestamp::from_millis(now + BACKOFF_MILLIS),
                            )
                            .expect("release");
                    }
                    // Succeeded: record the deed and count the execution. Normally settle and
                    // mark done; if this step is injected to lapse once, drop the settlement
                    // (do not fulfill or mark done) so the lease lapses and it is reclaimed —
                    // shaahid then suppresses the re-execution (exactly-once).
                    StepOutcome::Succeeded => {
                        *executions.entry(sigil.clone()).or_insert(0) += 1;
                        ledger.push(deed);
                        if !lapse_once.remove(&sigil) {
                            done.insert(sigil.clone());
                            registry.fulfill(&claim.retainer).expect("fulfill");
                        }
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

    #[test]
    fn the_same_composition_runs_over_the_durable_sqlite_backend() {
        // Backend-agnostic: the identical loop, over a real SQLite Registry, converges with
        // the same exactly-once and retry behavior — now durable.
        let report = run_with(
            vec![
                StepSpec::new("step:normal", StepMode::Normal),
                StepSpec::new("step:fails-first", StepMode::FailsFirst),
                StepSpec::new("step:lapses", StepMode::LapsesAfterSuccess),
            ],
            crate::store::SqliteRegistry::seeded,
        );

        assert!(
            report.converged,
            "must converge over SqliteRegistry: {report:?}"
        );
        for id in ["step:normal", "step:fails-first", "step:lapses"] {
            assert_eq!(
                report.executions.get(id).copied(),
                Some(1),
                "{id} must execute exactly once over SqliteRegistry: {report:?}"
            );
        }
        assert!(report.retried.contains("step:fails-first"), "{report:?}");
        assert!(report.deduplicated.contains("step:lapses"), "{report:?}");
    }

    #[cfg(unix)]
    mod agent {
        use super::*;
        use crate::agent::SubprocessAdapter;
        use std::os::unix::fs::PermissionsExt;
        use std::path::{Path, PathBuf};

        // Write an executable fake-agent script into `dir` and return its path.
        fn script_in(dir: &Path, name: &str, body: &str) -> PathBuf {
            std::fs::create_dir_all(dir).unwrap();
            let path = dir.join(name);
            std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
            path
        }

        fn workspace(tag: &str) -> PathBuf {
            std::env::temp_dir().join(format!("ringi-recon-{}-{tag}", std::process::id()))
        }

        fn runner(script: &Path, workspace: PathBuf) -> AgentStepRunner<SubprocessAdapter> {
            AgentStepRunner::new(
                SubprocessAdapter::new(script.to_string_lossy().to_string(), Vec::new()),
                workspace,
                Duration::from_secs(5),
            )
        }

        #[test]
        fn a_builder_agent_step_converges() {
            let ws = workspace("ok");
            let script = script_in(&ws, "builder-ok.sh", "echo '{\"status\":\"done\"}'\nexit 0");
            let report = run_with_runner(
                vec![StepSpec::new("step:build", StepMode::Normal)],
                MemoryRegistry::seeded,
                &runner(&script, ws.clone()),
            );

            assert!(
                report.converged,
                "the agent-driven run must converge: {report:?}"
            );
            assert_eq!(
                report.executions.get("step:build").copied(),
                Some(1),
                "the Builder step must execute exactly once: {report:?}"
            );
        }

        #[test]
        fn a_failing_builder_agent_retries_then_converges() {
            let ws = workspace("flaky");
            let marker = ws.join("attempted");
            // A real non-zero exit on the first attempt drives the loop's deferred reclaim; the
            // second attempt (marker present) exits clean.
            let script = script_in(
                &ws,
                "builder-flaky.sh",
                &format!(
                    "if [ -e '{m}' ]; then exit 0; else touch '{m}'; exit 1; fi",
                    m = marker.display()
                ),
            );
            let report = run_with_runner(
                vec![StepSpec::new("step:flaky", StepMode::Normal)],
                MemoryRegistry::seeded,
                &runner(&script, ws.clone()),
            );

            assert!(report.converged, "must converge after retrying: {report:?}");
            assert!(
                report.retried.contains("step:flaky"),
                "a real non-zero exit must drive the retry: {report:?}"
            );
            assert_eq!(
                report.executions.get("step:flaky").copied(),
                Some(1),
                "the step must execute exactly once (only the successful attempt): {report:?}"
            );
        }
    }
}
