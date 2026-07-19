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

use crate::agent::{AgentAdapter, AgentError, AgentRequest, AgentResponse, AgentRole};

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

/// Map an agent invocation result to a [`StepOutcome`]: a clean exit (a zero exit code) is
/// success; any other result — a non-zero exit, a spawn failure, or a timeout — is a failure the
/// loop retries. The caller computes no backoff — that is pacta's, loop-driven. Shared by the
/// per-step [`AgentStepRunner`] and the per-round [`AgentRoundBuilder`], which are distinct role
/// seams that nonetheless read an agent's result the same way.
fn outcome_of(result: Result<AgentResponse, AgentError>) -> StepOutcome {
    match result {
        Ok(response) if response.exit_code == Some(0) => StepOutcome::Succeeded,
        _ => StepOutcome::Failed,
    }
}

impl<A: AgentAdapter> StepRunner for AgentStepRunner<A> {
    fn run(&self, step: &StepSpec) -> StepOutcome {
        let request = AgentRequest {
            role: AgentRole::Builder,
            session_instruction: None,
            prompt: format!("Perform step: {}", step.id),
            working_dir: self.workspace.clone(),
            timeout: self.timeout,
            env: HashMap::new(),
        };
        outcome_of(self.adapter.run(request))
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

// ---------------------------------------------------------------------------------------------
// The round loop: propose -> verify -> review -> converge.
//
// Three roles, each its own seam (never one composable trait — that is the mirrorlane `Step`
// engine returning; see docs/round-model.md "Why this is not mirrorlane redux"): a Builder
// proposes, ringi's own Verification objectively certifies the goal, and a Reviewer surfaces
// findings. The goal `G` and each open finding are suunta targets; convergence stays suunta's.
// ---------------------------------------------------------------------------------------------

/// The objective verdict on the goal. Ringi re-runs the checks itself; an agent never decides
/// this (PROJECT.md: tool verification outranks model opinion).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// The goal is objectively met.
    Pass,
    /// The goal is not met; it stays in the residual for another round.
    Fail,
}

/// Ringi's own objective certifier of the goal `G`. This increment's implementations are
/// scripted; a real command runner (build/test/lint) lands in a later change. It is ringi's own
/// blood, not a brick.
pub trait Verification {
    /// Certify whether the goal is objectively met at `round`.
    fn verify(&self, round: usize) -> Verdict;
}

/// A single review finding — a defect the Reviewer raises. `id` is stable across rounds, so the
/// same finding keeps one target identity until a re-review no longer reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// Stable identity of the finding across rounds.
    pub id: String,
    /// Human-readable description (rides in the pact clause; ringi does not interpret it).
    pub summary: String,
}

/// The seam by which ringi scrutinizes a build result. A review is read-only and returns the
/// full set of currently-open findings; a finding absent from a later review is resolved.
pub trait ReviewRunner {
    /// Review the state at `round`, returning every finding still open.
    fn review(&self, round: usize) -> Vec<Finding>;
}

/// The seam by which ringi performs a round's build work toward the goal and open findings.
/// Distinct from [`ReviewRunner`]/[`Verification`] — three roles, not one trait.
pub trait RoundBuilder {
    /// Perform `round`'s build work and report whether the attempt succeeded.
    fn build(&self, round: usize) -> StepOutcome;
}

/// Performs a round's build work by running a Builder agent through the agent seam. The round-loop
/// counterpart to [`AgentStepRunner`] — a distinct role seam (`RoundBuilder`, not `StepRunner`),
/// never one composable trait. Depends only on the [`AgentAdapter`], so any adapter backs it.
#[derive(Debug, Clone)]
pub struct AgentRoundBuilder<A> {
    adapter: A,
    task: String,
    workspace: PathBuf,
    timeout: Duration,
}

impl<A: AgentAdapter> AgentRoundBuilder<A> {
    /// A Builder round runner over `adapter` for `task`, building each round in `workspace`
    /// bounded by `timeout`. The task seeds the Builder prompt, so the agent is asked to carry it
    /// out.
    pub fn new(
        adapter: A,
        task: impl Into<String>,
        workspace: impl Into<PathBuf>,
        timeout: Duration,
    ) -> Self {
        Self {
            adapter,
            task: task.into(),
            workspace: workspace.into(),
            timeout,
        }
    }
}

impl<A: AgentAdapter> RoundBuilder for AgentRoundBuilder<A> {
    fn build(&self, round: usize) -> StepOutcome {
        let request = AgentRequest {
            role: AgentRole::Builder,
            session_instruction: None,
            // Task-aware: the run's task rides in the prompt. Conveying the round's open findings
            // is a later change; `build` still takes only the round.
            prompt: format!("Task: {}\n\nPerform build for round {round}.", self.task),
            working_dir: self.workspace.clone(),
            timeout: self.timeout,
            env: HashMap::new(),
        };
        // A clean exit is a successful build; anything else is a failed attempt the loop retries
        // via deferred reclaim. No backoff here — that is pacta's, driven by the loop.
        outcome_of(self.adapter.run(request))
    }
}

/// A read-only Reviewer backed by an [`AgentAdapter`]: it runs a Reviewer agent and parses the
/// findings from its structured output. Depends only on the seam, so any adapter backs it.
#[derive(Debug, Clone)]
pub struct AgentReviewRunner<A> {
    adapter: A,
    workspace: PathBuf,
    timeout: Duration,
}

impl<A: AgentAdapter> AgentReviewRunner<A> {
    /// A Reviewer runner over `adapter`, reviewing in `workspace` bounded by `timeout`.
    pub fn new(adapter: A, workspace: impl Into<PathBuf>, timeout: Duration) -> Self {
        Self {
            adapter,
            workspace: workspace.into(),
            timeout,
        }
    }
}

/// Parse findings from a Reviewer agent's structured output: `{"findings":[{"id","summary"}]}`.
/// Best-effort — a missing or malformed shape yields no findings.
fn parse_findings(structured: Option<serde_json::Value>) -> Vec<Finding> {
    let Some(value) = structured else {
        return Vec::new();
    };
    let Some(array) = value.get("findings").and_then(|f| f.as_array()) else {
        return Vec::new();
    };
    array
        .iter()
        .filter_map(|f| {
            let id = f.get("id")?.as_str()?.to_string();
            let summary = f
                .get("summary")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string();
            Some(Finding { id, summary })
        })
        .collect()
}

impl<A: AgentAdapter> ReviewRunner for AgentReviewRunner<A> {
    fn review(&self, round: usize) -> Vec<Finding> {
        let request = AgentRequest {
            role: AgentRole::Reviewer,
            session_instruction: None,
            prompt: format!("Review the work for round {round} and report findings."),
            working_dir: self.workspace.clone(),
            timeout: self.timeout,
            env: HashMap::new(),
        };
        // A read-only review: parse findings on success. An infrastructure failure surfaces no
        // findings here (real error handling — not treating a failed review as a clean one — is
        // a later change; this increment's tests drive successful reviews).
        match self.adapter.run(request) {
            Ok(response) => parse_findings(response.metadata),
            Err(_) => Vec::new(),
        }
    }
}

/// Thin seam adapters — mapping only. Any logic here beyond translation is the monolith
/// returning.
mod seam {
    use super::{Correction, Deed, Finding, Reversibility, Sigil, StepSpec};
    use pacta::Pact;
    use shaahid::Fingerprint;
    use uuid::Uuid;

    /// The exactly-once identity of a single **attempt** — a coordinate distinct from a
    /// target's `Sigil` (round-model ①). suunta reasons about targets (`Sigil`); shaahid/pacta
    /// reason about attempts (`Seal`). The coordinate is `<run>:<target>:<round>:<attempt>`:
    /// a reclaim of the same attempt re-presents the same `Seal` (witnessed as already done),
    /// while a new round is a new coordinate and therefore new work. The flat reconcile has no
    /// rounds, so it uses a degenerate coordinate (round 0, attempt 0); the round loop supplies
    /// real coordinates.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Seal(String);

    impl Seal {
        /// The attempt coordinate for `target` in `run` at `round`/`attempt`. shaahid compares
        /// the whole `Seal` by value; ringi never parses the string back.
        #[must_use]
        pub fn new(run: &str, target: &Sigil, round: usize, attempt: usize) -> Self {
            Self(format!("{run}:{}:{round}:{attempt}", target.as_str()))
        }
    }

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

    /// A step attempt becomes a `Deed`: the `Seal` is the attempt coordinate (distinct from the
    /// target `Sigil`), the fingerprint is the content identity of the step spec. The flat
    /// reconcile has no rounds, so the coordinate is degenerate (round 0, attempt 0) yet stable
    /// per target — a reclaim after success re-presents the same `Seal` and attaches.
    pub fn deed_for_step(correction: &Correction<StepSpec>) -> Deed<Seal> {
        let sigil = correction.sigil();
        Deed::new(
            Seal::new("reconcile", sigil, 0, 0),
            Fingerprint::new(correction.body().id.as_bytes().to_vec()),
        )
    }

    // ---- Round-loop mappings ---------------------------------------------------------------

    /// The goal target's stable identity.
    pub fn goal_sigil() -> Sigil {
        Sigil::new("goal")
    }

    /// A finding's stable target identity, held across rounds until a re-review drops it.
    pub fn finding_sigil(finding: &Finding) -> Sigil {
        Sigil::new(format!("finding:{}", finding.id))
    }

    /// The goal as a suunta target (payload unused — per-round mode does not execute per-target).
    pub fn goal_correction() -> Correction<()> {
        Correction::new(goal_sigil(), Reversibility::Reversible, ())
    }

    /// An open finding as a suunta target.
    pub fn finding_correction(finding: &Finding) -> Correction<()> {
        Correction::new(finding_sigil(finding), Reversibility::Reversible, ())
    }

    /// The docket a round's build attempt is claimed by.
    pub fn build_docket(run: &str, round: usize) -> String {
        format!("{run}:build:{round}")
    }

    /// The pact for a round's build attempt.
    pub fn build_pact(run: &str, round: usize) -> Pact {
        let docket = build_docket(run, round);
        let id = Uuid::new_v5(&Uuid::NAMESPACE_OID, docket.as_bytes());
        Pact::new(id, docket.clone(), "build".to_string(), docket.into_bytes())
    }

    /// The `Deed` witnessing a round's build attempt: the `Seal` is the attempt coordinate
    /// (`<run>:build:<round>:<attempt>`), the fingerprint is over the attempt's input identity.
    pub fn build_deed(run: &str, round: usize, attempt: usize) -> Deed<Seal> {
        let seal = Seal::new(run, &Sigil::new("build"), round, attempt);
        Deed::new(
            seal,
            Fingerprint::new(build_docket(run, round).into_bytes()),
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
    let mut ledger: Vec<Deed<seam::Seal>> = Vec::new(); // shaahid: attempts whose work succeeded
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

/// Consumer-held bound on rounds (Layer-3 termination — a review can always find more, so the
/// loop must not depend on the Reviewer ever going quiet).
const MAX_ROUNDS: usize = 16;

/// The observable outcome of a round-loop run, for asserting the composition.
#[derive(Debug)]
pub struct RoundReport {
    /// Rounds the loop ran.
    pub rounds: usize,
    /// Whether suunta reported the run converged (vs. hitting the round bound).
    pub converged: bool,
    /// Per-round count of how many times the build side effect executed (exactly-once ⇒ 1).
    pub build_executions: HashMap<usize, u32>,
    /// Rounds whose build attempt was reclaimed after a lost settlement and suppressed by
    /// shaahid (attached, not re-executed).
    pub reclaimed: HashSet<usize>,
    /// The Bearing size at each round, so a test can observe the target set changing.
    pub bearing_sizes: Vec<usize>,
    /// Finding ids still open when the loop ended.
    pub open_findings: Vec<String>,
}

/// The durable checkpoint seam: the round loop notifies its journal so a run's progress survives
/// the process and can be resumed. It is ringi's own durable state (the shaahid `witness` function
/// stays sans-I/O) — the loop never reads it back; a resume is reconstructed via [`Resume`]. The
/// no-op implementation for `()` is what the in-memory composition uses, so that path is unchanged.
pub trait RunJournal {
    /// A round's build attempt succeeded. Called **before** the pact is settled, so the deed is
    /// durable even if the process dies in the settlement window.
    fn build_succeeded(&self, round: usize);
    /// A round completed without convergence: the run should resume at `next_round` with these
    /// findings still open.
    fn checkpoint(&self, next_round: usize, open_findings: &[Finding]);
}

/// The no-op journal — the in-memory composition records nothing and behaves exactly as before.
impl RunJournal for () {
    fn build_succeeded(&self, _round: usize) {}
    fn checkpoint(&self, _next_round: usize, _open_findings: &[Finding]) {}
}

/// A resume point loaded from a durable journal: where to re-enter the loop and the state to
/// restore. `built_rounds` reconstructs the witness ledger (a build `Deed` is determined by
/// `(run, round)`), so a reclaimed in-flight attempt attaches instead of re-executing.
#[derive(Debug, Clone)]
pub struct Resume {
    /// The round to re-enter at (completed rounds are not revisited).
    pub start_round: usize,
    /// Rounds whose build already succeeded — used to rebuild the ledger.
    pub built_rounds: Vec<usize>,
    /// Findings still open at the checkpoint.
    pub open_findings: Vec<Finding>,
}

/// Run the round loop over any backend built by `make`, driving Build → Verify → (green?)
/// Review until suunta reports convergence or the round bound is hit. Production entry point
/// (no fault injection). The goal `G` and each open finding are suunta targets; the goal's
/// satisfaction is the `Verification` verdict and never the Reviewer's output; convergence is
/// suunta's alone. Build/Review/Verify are three concrete roles wired here — never one
/// composable trait (that is the mirrorlane `Step` engine returning).
pub fn run_rounds<B, Rv, V, F, Reg>(
    run_id: &str,
    builder: &B,
    reviewer: &Rv,
    verification: &V,
    make: F,
) -> RoundReport
where
    B: RoundBuilder,
    Rv: ReviewRunner,
    V: Verification,
    Reg: Registry,
    Reg::Error: std::fmt::Debug,
    F: FnOnce(Vec<Pact>, u64) -> Reg,
{
    run_rounds_core(
        run_id,
        builder,
        reviewer,
        verification,
        make,
        HashSet::new(),
        &(),
        None,
    )
}

/// Run the round loop with a durable `journal` (recording progress) and an optional `resume`
/// point (re-entering an interrupted run from its checkpoint). This is the entry point the
/// durable CLI drives: `resume = None` for a fresh run, `Some(..)` to continue an interrupted one.
/// The loop body is identical to [`run_rounds`]; only checkpointing and re-entry differ.
pub fn run_rounds_journaled<B, Rv, V, F, Reg>(
    run_id: &str,
    builder: &B,
    reviewer: &Rv,
    verification: &V,
    make: F,
    journal: &dyn RunJournal,
    resume: Option<Resume>,
) -> RoundReport
where
    B: RoundBuilder,
    Rv: ReviewRunner,
    V: Verification,
    Reg: Registry,
    Reg::Error: std::fmt::Debug,
    F: FnOnce(Vec<Pact>, u64) -> Reg,
{
    run_rounds_core(
        run_id,
        builder,
        reviewer,
        verification,
        make,
        HashSet::new(),
        journal,
        resume,
    )
}

/// The round loop body, parameterized over the backend and the (test-only) set of rounds whose
/// build settlement is lost once — to exercise reclaim + exactly-once at the attempt grain.
// Internal orchestration helper: the roles, backend, fault-injection, journal, and resume point
// are each distinct and generic, so bundling them buys no clarity.
#[allow(clippy::too_many_arguments)]
fn run_rounds_core<B, Rv, V, F, Reg>(
    run_id: &str,
    builder: &B,
    reviewer: &Rv,
    verification: &V,
    make: F,
    mut lapse_rounds: HashSet<usize>,
    journal: &dyn RunJournal,
    resume: Option<Resume>,
) -> RoundReport
where
    B: RoundBuilder,
    Rv: ReviewRunner,
    V: Verification,
    Reg: Registry,
    Reg::Error: std::fmt::Debug,
    F: FnOnce(Vec<Pact>, u64) -> Reg,
{
    // pacta-memory has no runtime enqueue, so seed one build pact per possible round upfront
    // (ingress is the backend's; a durable backend would create them idempotently at runtime).
    let pacts: Vec<Pact> = (0..MAX_ROUNDS)
        .map(|r| seam::build_pact(run_id, r))
        .collect();
    let registry = make(pacts, LEASE_MILLIS);

    let mut ledger: Vec<Deed<seam::Seal>> = Vec::new();
    let mut open_findings: Vec<Finding> = Vec::new();
    let mut build_executions: HashMap<usize, u32> = HashMap::new();
    let mut reclaimed: HashSet<usize> = HashSet::new();
    let mut bearing_sizes: Vec<usize> = Vec::new();

    let mut now = 0u64;
    let mut rounds = 0usize;
    let mut converged = false;

    // Resume: rebuild the witness ledger from the recorded succeeded rounds (a build Deed is
    // determined by (run, round)), restore the open findings, and re-enter at the checkpoint —
    // so completed rounds are not revisited and a reclaimed attempt attaches, not re-executes.
    if let Some(resume) = resume {
        for r in resume.built_rounds {
            ledger.push(seam::build_deed(run_id, r, 0));
        }
        open_findings = resume.open_findings;
        rounds = resume.start_round;
    }

    while rounds < MAX_ROUNDS {
        let round = rounds;
        rounds += 1;

        // 1. This round's build attempt — per-round, durable (pacta) and exactly-once (shaahid).
        let (execs, was_reclaimed) = drive_build(
            &registry,
            &mut ledger,
            run_id,
            round,
            builder,
            &mut now,
            lapse_rounds.remove(&round),
            journal,
        );
        *build_executions.entry(round).or_insert(0) += execs;
        if was_reclaimed {
            reclaimed.insert(round);
        }

        // 2. Verify certifies the goal — objective, never the Reviewer's opinion.
        let verdict = verification.verify(round);

        // 3. Review only a green build; a red goal skips review and drives another round.
        if verdict == Verdict::Pass {
            open_findings = reviewer.review(round);
        }

        // 4. Present the changing Bearing to suunta and let it decide convergence.
        let targets: Vec<Correction<()>> = std::iter::once(seam::goal_correction())
            .chain(open_findings.iter().map(seam::finding_correction))
            .collect();
        bearing_sizes.push(targets.len());

        let goal_satisfaction = match verdict {
            Verdict::Pass => Satisfaction::Satisfied,
            Verdict::Fail => Satisfaction::Unsatisfied,
        };
        let mut fix = vec![SatisfactionFinding {
            target: seam::goal_sigil(),
            satisfaction: goal_satisfaction,
        }];
        // An open finding is Unsatisfied by definition; a resolved one is simply absent.
        for finding in &open_findings {
            fix.push(SatisfactionFinding {
                target: seam::finding_sigil(finding),
                satisfaction: Satisfaction::Unsatisfied,
            });
        }
        let sounding = Sounding::new(Fix::new(fix), Vec::new());
        let residual = plan_residual(Bearing::new(targets), &sounding);
        if residual.is_converged() {
            converged = true;
            break;
        }
        // Not converged: checkpoint so a resume re-enters at the next round with these findings.
        journal.checkpoint(rounds, &open_findings);
    }

    RoundReport {
        rounds,
        converged,
        build_executions,
        reclaimed,
        bearing_sizes,
        open_findings: open_findings.into_iter().map(|f| f.id).collect(),
    }
}

/// Drive one round's build attempt to a settled state, composing pacta (claim/settle) and
/// shaahid (exactly-once). Returns how many times the build side effect executed (0 or 1) and
/// whether it was reclaimed after a lost settlement. If `lapse`, the first successful settlement
/// is dropped, so the lease lapses and the attempt is reclaimed — shaahid then attaches and the
/// build is not re-executed.
// Internal helper: each argument is a distinct role/coordinate; bundling buys no clarity.
#[allow(clippy::too_many_arguments)]
fn drive_build<B, Reg>(
    registry: &Reg,
    ledger: &mut Vec<Deed<seam::Seal>>,
    run_id: &str,
    round: usize,
    builder: &B,
    now: &mut u64,
    mut lapse: bool,
    journal: &dyn RunJournal,
) -> (u32, bool)
where
    B: RoundBuilder,
    Reg: Registry,
    Reg::Error: std::fmt::Debug,
{
    let docket = seam::build_docket(run_id, round);
    let mut execs = 0u32;
    let mut reclaimed = false;

    // Bounded so a persistently failing scripted build cannot spin forever.
    for _ in 0..8 {
        let Some(claim) = registry
            .claim(&[docket.as_str()], Timestamp::from_millis(*now))
            .expect("build claim never errors here")
        else {
            // Withheld (released or lapsed, not yet reclaimable) — advance and retry.
            *now += TICK_MILLIS;
            continue;
        };

        let deed = seam::build_deed(run_id, round, 0);
        let outcome = witness(ledger, deed.clone());
        if !outcome.contradictions.is_empty() {
            // Deterministic deeds here raise none; wired for correctness.
            *now += TICK_MILLIS;
            continue;
        }

        match outcome.attestation {
            // A reclaim after a lost settlement: the attempt already ran — do not re-run it.
            Attestation::Attach(_) => {
                reclaimed = true;
                registry.fulfill(&claim.retainer).expect("fulfill");
                break;
            }
            Attestation::Create => match builder.build(round) {
                StepOutcome::Succeeded => {
                    execs += 1;
                    ledger.push(deed);
                    // Journal the success BEFORE settling, so a crash in the settlement window
                    // leaves a durable deed and the reclaim on resume attaches, not re-executes.
                    journal.build_succeeded(round);
                    if lapse {
                        // Drop this settlement: do not fulfill. Advance past the lease so the
                        // pact lapses and is reclaimed next iteration -> witnessed as Attach.
                        lapse = false;
                        *now += LEASE_MILLIS + TICK_MILLIS;
                        continue;
                    }
                    registry.fulfill(&claim.retainer).expect("fulfill");
                    break;
                }
                StepOutcome::Failed => {
                    registry
                        .release(
                            &claim.retainer,
                            Timestamp::from_millis(*now + BACKOFF_MILLIS),
                        )
                        .expect("release");
                    *now += TICK_MILLIS;
                }
            },
        }
    }

    *now += TICK_MILLIS;
    (execs, reclaimed)
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

    // ---- Round loop -----------------------------------------------------------------------

    /// A scripted Builder: every attempt "runs". Whether the goal is met is Verify's call.
    struct ScriptBuild;
    impl RoundBuilder for ScriptBuild {
        fn build(&self, _round: usize) -> StepOutcome {
            StepOutcome::Succeeded
        }
    }

    /// A scripted Verify: red until `pass_from`, then green.
    struct ScriptVerify {
        pass_from: usize,
    }
    impl Verification for ScriptVerify {
        fn verify(&self, round: usize) -> Verdict {
            if round >= self.pass_from {
                Verdict::Pass
            } else {
                Verdict::Fail
            }
        }
    }

    /// A scripted Reviewer: returns the finding set scheduled for the round (empty past the end).
    struct ScriptReview {
        schedule: Vec<Vec<Finding>>,
    }
    impl ReviewRunner for ScriptReview {
        fn review(&self, round: usize) -> Vec<Finding> {
            self.schedule.get(round).cloned().unwrap_or_default()
        }
    }

    /// A scripted Reviewer that surfaces the same findings every round (never resolves them).
    struct ConstReview {
        findings: Vec<Finding>,
    }
    impl ReviewRunner for ConstReview {
        fn review(&self, _round: usize) -> Vec<Finding> {
            self.findings.clone()
        }
    }

    fn finding(id: &str) -> Finding {
        Finding {
            id: id.to_string(),
            summary: format!("resolve {id}"),
        }
    }

    #[test]
    fn the_round_loop_converges_over_a_changing_bearing() {
        // round 0: goal not yet met (red) -> no review.
        // round 1: goal met (green) -> review surfaces F1, F2.
        // round 2: goal met, review clean -> converge.
        let reviewer = ScriptReview {
            schedule: vec![
                Vec::new(),                         // round 0 (not consulted: red)
                vec![finding("F1"), finding("F2")], // round 1
                Vec::new(),                         // round 2: resolved
            ],
        };
        let report = run_rounds(
            "run-converge",
            &ScriptBuild,
            &reviewer,
            &ScriptVerify { pass_from: 1 },
            MemoryRegistry::seeded,
        );

        assert!(report.converged, "the round loop must converge: {report:?}");
        assert_eq!(report.rounds, 3, "red, findings, then clean: {report:?}");
        // The Bearing changed as findings appeared then resolved.
        assert_eq!(report.bearing_sizes, vec![1, 3, 1], "{report:?}");
        for r in 0..3 {
            assert_eq!(
                report.build_executions.get(&r).copied(),
                Some(1),
                "each round built exactly once: {report:?}"
            );
        }
        assert!(report.open_findings.is_empty(), "{report:?}");
    }

    #[test]
    fn no_findings_but_red_verify_does_not_converge() {
        // The Reviewer never surfaces anything, but Verify never passes: completion is Verify's
        // verdict, never the absence of findings, so the run does not converge.
        let report = run_rounds(
            "run-red",
            &ScriptBuild,
            &ScriptReview {
                schedule: Vec::new(),
            },
            &ScriptVerify {
                pass_from: usize::MAX,
            },
            MemoryRegistry::seeded,
        );
        assert!(
            !report.converged,
            "a red goal must not converge: {report:?}"
        );
        assert_eq!(
            report.rounds, MAX_ROUNDS,
            "bounded, not infinite: {report:?}"
        );
    }

    #[test]
    fn green_verify_but_open_finding_does_not_converge() {
        // The goal is green from the start, but a finding is never resolved: an open finding is a
        // real target, so it keeps the run from converging.
        let report = run_rounds(
            "run-open",
            &ScriptBuild,
            &ConstReview {
                findings: vec![finding("F1")],
            },
            &ScriptVerify { pass_from: 0 },
            MemoryRegistry::seeded,
        );
        assert!(
            !report.converged,
            "an open finding must block convergence: {report:?}"
        );
        assert_eq!(report.rounds, MAX_ROUNDS, "{report:?}");
        assert_eq!(report.open_findings, vec!["F1".to_string()], "{report:?}");
    }

    #[test]
    fn a_reclaimed_build_attempt_is_not_re_executed() {
        // The converging scenario, but round 1's build settlement is lost: the lease lapses, the
        // attempt is reclaimed, and shaahid attaches -> the build is not re-executed.
        let reviewer = ScriptReview {
            schedule: vec![Vec::new(), vec![finding("F1")], Vec::new()],
        };
        let report = run_rounds_core(
            "run-reclaim",
            &ScriptBuild,
            &reviewer,
            &ScriptVerify { pass_from: 1 },
            MemoryRegistry::seeded,
            HashSet::from([1usize]),
            &(),
            None,
        );

        assert!(
            report.converged,
            "must still converge after a reclaim: {report:?}"
        );
        assert!(
            report.reclaimed.contains(&1),
            "round 1's attempt was reclaimed: {report:?}"
        );
        assert_eq!(
            report.build_executions.get(&1).copied(),
            Some(1),
            "round 1 built exactly once despite the reclaim: {report:?}"
        );
    }

    #[test]
    fn a_resumed_run_re_enters_at_the_checkpoint_round() {
        // Resume at round 2 with rounds 0 and 1 already built: the loop must not revisit them.
        let reviewer = ScriptReview {
            schedule: vec![Vec::new(); 5],
        };
        let resume = Resume {
            start_round: 2,
            built_rounds: vec![0, 1],
            open_findings: Vec::new(),
        };
        let report = run_rounds_journaled(
            "run-resume-offset",
            &ScriptBuild,
            &reviewer,
            &ScriptVerify { pass_from: 2 },
            MemoryRegistry::seeded,
            &(),
            Some(resume),
        );
        assert!(report.converged, "a resumed run converges: {report:?}");
        assert!(
            !report.build_executions.contains_key(&0) && !report.build_executions.contains_key(&1),
            "completed rounds must not be re-entered: {report:?}"
        );
        assert_eq!(
            report.build_executions.get(&2).copied(),
            Some(1),
            "the resume round runs its build: {report:?}"
        );
    }

    #[test]
    fn a_recorded_build_is_not_re_executed_on_resume() {
        // The crash-in-settlement-window case: round 0's build succeeded (recorded) but was not
        // settled, so on resume round 0 is re-entered. The ledger — reconstructed from
        // `built_rounds` — makes the attempt attach instead of re-executing.
        let reviewer = ScriptReview {
            schedule: vec![Vec::new()],
        };
        let resume = Resume {
            start_round: 0,
            built_rounds: vec![0],
            open_findings: Vec::new(),
        };
        let report = run_rounds_journaled(
            "run-resume-attach",
            &ScriptBuild,
            &reviewer,
            &ScriptVerify { pass_from: 0 },
            MemoryRegistry::seeded,
            &(),
            Some(resume),
        );
        assert!(report.converged, "must converge: {report:?}");
        assert_eq!(
            report.build_executions.get(&0).copied(),
            Some(0),
            "a build recorded before the crash must attach, not re-execute: {report:?}"
        );
        assert!(
            report.reclaimed.contains(&0),
            "round 0 attaches to the recorded deed: {report:?}"
        );
    }

    /// A journal that captures the loop's checkpoints and recorded deeds in memory, for asserting
    /// what would be persisted.
    #[derive(Default)]
    struct CapturingJournal {
        built: std::cell::RefCell<Vec<usize>>,
        last_checkpoint: std::cell::RefCell<Option<usize>>,
    }
    impl RunJournal for CapturingJournal {
        fn build_succeeded(&self, round: usize) {
            self.built.borrow_mut().push(round);
        }
        fn checkpoint(&self, next_round: usize, _open: &[Finding]) {
            *self.last_checkpoint.borrow_mut() = Some(next_round);
        }
    }

    #[test]
    fn a_run_journals_its_builds_and_checkpoints() {
        // Two rounds before convergence: the journal sees each build and a checkpoint advancing
        // the next round, so an interrupted run would have a resume point.
        let reviewer = ScriptReview {
            schedule: vec![vec![finding("F1")], Vec::new()],
        };
        let journal = CapturingJournal::default();
        let report = run_rounds_journaled(
            "run-journal",
            &ScriptBuild,
            &reviewer,
            &ScriptVerify { pass_from: 0 },
            MemoryRegistry::seeded,
            &journal,
            None,
        );
        assert!(report.converged, "{report:?}");
        assert!(
            journal.built.borrow().contains(&0),
            "the first build is journalled before settlement"
        );
        assert!(
            journal.last_checkpoint.borrow().is_some(),
            "a non-converged round checkpoints the next round"
        );
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

        fn review_runner(
            script: &Path,
            workspace: PathBuf,
        ) -> AgentReviewRunner<SubprocessAdapter> {
            AgentReviewRunner::new(
                SubprocessAdapter::new(script.to_string_lossy().to_string(), Vec::new()),
                workspace,
                Duration::from_secs(5),
            )
        }

        #[test]
        fn an_agent_reviewer_parses_findings_from_structured_output() {
            let ws = workspace("review");
            let script = script_in(
                &ws,
                "reviewer.sh",
                "echo '{\"findings\":[{\"id\":\"F1\",\"summary\":\"bug\"},{\"id\":\"F2\",\"summary\":\"nit\"}]}'\nexit 0",
            );
            let findings = review_runner(&script, ws.clone()).review(0);
            assert_eq!(findings.len(), 2, "two findings parsed: {findings:?}");
            assert_eq!(findings[0].id, "F1");
            assert_eq!(findings[1].id, "F2");
        }

        #[test]
        fn an_agent_reviewer_with_no_findings_is_clean() {
            let ws = workspace("review-clean");
            let script = script_in(&ws, "reviewer-clean.sh", "echo '{\"findings\":[]}'\nexit 0");
            assert!(
                review_runner(&script, ws.clone()).review(0).is_empty(),
                "an empty findings array is a clean review"
            );
        }

        fn round_builder(
            script: &Path,
            workspace: PathBuf,
        ) -> AgentRoundBuilder<SubprocessAdapter> {
            AgentRoundBuilder::new(
                SubprocessAdapter::new(script.to_string_lossy().to_string(), Vec::new()),
                "do the thing",
                workspace,
                Duration::from_secs(5),
            )
        }

        #[test]
        fn the_round_loop_converges_under_agent_backed_builder_and_reviewer() {
            // End to end under real agents: a Builder agent that always exits clean, and a
            // Reviewer agent that surfaces F1 on its first review then none thereafter (a marker
            // in the shared workspace). Verify is a scripted green, so the test isolates the two
            // *agent* roles being wired into the loop.
            let ws = workspace("rounds-e2e");
            let builder = script_in(
                &ws,
                "round-builder.sh",
                "echo '{\"status\":\"built\"}'\nexit 0",
            );
            let reviewer = script_in(
                &ws,
                "round-reviewer.sh",
                "if [ -e reviewed ]; then echo '{\"findings\":[]}'; else touch reviewed; \
                 echo '{\"findings\":[{\"id\":\"F1\",\"summary\":\"fix\"}]}'; fi\nexit 0",
            );

            let report = run_rounds(
                "run-agent-e2e",
                &round_builder(&builder, ws.clone()),
                &review_runner(&reviewer, ws.clone()),
                &ScriptVerify { pass_from: 0 },
                MemoryRegistry::seeded,
            );

            assert!(
                report.converged,
                "the agent-backed round loop must converge: {report:?}"
            );
            // Goal green from round 0; F1 surfaces round 0 then resolves round 1 -> converge.
            for r in 0..report.rounds {
                assert_eq!(
                    report.build_executions.get(&r).copied(),
                    Some(1),
                    "each round built exactly once: {report:?}"
                );
            }
            assert!(
                report.open_findings.is_empty(),
                "findings must resolve: {report:?}"
            );
        }
    }
}
