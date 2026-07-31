# reconcile-loop Specification

## Purpose

The consumer loop that composes suunta (residual planning and convergence), shaahid
(exactly-once step execution), and pacta (durable step lifecycle) into a convergent,
idempotent, durable reconcile of a desired set of steps — the orchestrator emerging from
composition, with ringi only wiring.

## Requirements

### Requirement: The Reconcile Loop Composes The Family Over Public APIs
Ringi SHALL reconcile a desired set of steps to done through a consumer loop that composes
suunta (residual planning and convergence), shaahid (exactly-once step execution), and pacta
(durable step lifecycle) using only the public APIs of those crates. Ringi SHALL add no
step-lifecycle state machine, completion calculation, or idempotency scheme of its own — the
only ringi-owned logic is the loop and thin seam adapters (identity mapping, findings
translation). If a seam cannot be expressed via a brick's public API, that SHALL be recorded
as a finding, not worked around by reaching inside.

#### Scenario: The loop reconciles a desired set to done
- **WHEN** ringi is given a set of desired steps and runs the reconcile loop
- **THEN** it drives each step through pacta's claim/execute/settle, planning with suunta and witnessing with shaahid, until every step is done

#### Scenario: No brick behavior is reimplemented
- **WHEN** the reconcile loop needs planning, convergence, idempotency, or lifecycle
- **THEN** it calls the corresponding brick rather than computing that behavior itself

### Requirement: Convergence Is Decided By Suunta
The loop SHALL halt as complete only when suunta reports the residual converged, never by a
ringi-owned completion check. Each cycle SHALL supply suunta a `Sounding` of domain-certified
satisfaction and coverage findings and act on the returned residual.

#### Scenario: The run ends when suunta converges
- **WHEN** every desired step is satisfied and nothing is surfaced
- **THEN** `Residual::is_converged` is true and the loop stops as complete

#### Scenario: A retained step keeps the loop going
- **WHEN** the residual still contains a step
- **THEN** the loop performs another cycle rather than declaring completion

### Requirement: Each Step Executes Exactly Once
A step's side effect SHALL occur exactly once across the whole run, including retries, by
witnessing the step attempt with shaahid before performing it: a create-attestation performs
and records; an attach-attestation is a no-op. This is what makes ringi's executor idempotent
under pacta's at-least-once recovery.

#### Scenario: A retried step still runs once
- **WHEN** a step is executed, retried, and executed again through the loop
- **THEN** its side effect occurred exactly once because the second attempt attaches rather than re-performs

### Requirement: A Failed Step Retries Via Deferred Reclaim
A step that fails an attempt SHALL be retried by releasing its claim with a
consumer-computed reclaimable instant (`release(retainer, reclaimable_at)`), so it is
withheld until that instant and then reclaimed — the backoff policy is ringi's, the mechanism
is pacta's.

#### Scenario: A failed step is withheld then reclaimed
- **WHEN** a step fails its first attempt and the loop releases it with a future reclaimable instant
- **THEN** the step is not claimed before that instant and is reclaimed and completed after it

### Requirement: The Composition Is Self-Checked
The reconcile loop SHALL be exercised by a self-checking test that asserts the run converges,
each step executed exactly once, and a failed step was withheld then reclaimed, so the bet
cannot silently regress under the Definition of Done.

#### Scenario: A regressed composition fails the gate
- **WHEN** the loop no longer converges, double-executes a step, or mishandles retry
- **THEN** the self-checking test fails under the Definition of Done

### Requirement: Step Execution Is Delegated To A Runner Seam
The loop SHALL delegate a claimed step's actual work to a `StepRunner` seam and act only on
the outcome it returns — a success settles the step (record + fulfil), a failure retries it
(release with a reclaimable instant). The loop SHALL NOT itself decide whether a step's work
succeeded; that judgment belongs to the runner, keeping the loop pure composition. The
production runner runs an agent; a scripted runner drives the self-checking composition test.

#### Scenario: A step's outcome comes from its runner
- **WHEN** the loop executes a claimed step
- **THEN** it calls the runner, settles the step on a success outcome, and releases it for retry on a failure outcome, without judging the work itself

### Requirement: The Loop Reconciles A Changing Bearing Of Goal Plus Findings
The loop SHALL build its suunta `Bearing` each cycle from the current work set — the goal target
`G` together with the open review findings, each finding a `Correction` with a stable `Sigil` —
rather than from a fixed set decided once. A finding target SHALL be Unsatisfied until a
re-review certifies it resolved, at which point it SHALL drop from the Bearing; a newly surfaced
finding SHALL enter the Bearing. Convergence over this changing Bearing SHALL remain suunta's
decision, with the loop adding no completion logic of its own.

#### Scenario: A new finding enters the Bearing
- **WHEN** a Reviewer surfaces a finding not previously seen
- **THEN** the next cycle's Bearing includes that finding as an Unsatisfied target

#### Scenario: A resolved finding leaves the Bearing
- **WHEN** a re-review certifies a previously open finding resolved
- **THEN** that finding target is Satisfied and no longer appears in the residual

#### Scenario: The run converges on a clean review and a green goal
- **WHEN** the goal is objectively verified and a review surfaces no open findings
- **THEN** the residual is empty, nothing is surfaced, and suunta reports the run converged

### Requirement: Two Certifiers Feed The Sounding
Each cycle the loop SHALL assemble the `Sounding`'s `Fix` from two distinct certifiers: the goal
target `G`'s satisfaction SHALL come only from the `Verification` verdict, and each finding
target's satisfaction SHALL come only from a Reviewer re-review. The loop SHALL NOT satisfy `G`
from the absence of findings, nor satisfy a finding from a verification verdict — the two axes
stay separate.

#### Scenario: The goal's satisfaction comes from Verify
- **WHEN** the loop certifies the goal target for a cycle
- **THEN** its satisfaction is the `Verification` verdict, never the Reviewer's output

#### Scenario: A finding's satisfaction comes from re-review
- **WHEN** the loop certifies a finding target for a cycle
- **THEN** its satisfaction is a Reviewer re-review verdict, never the verification verdict

### Requirement: The Exactly-Once Attempt Identity Is Distinct From The Target Identity
The loop SHALL identify a suunta target by a stable `Sigil` and a shaahid attempt by a distinct
`Seal` coordinate (`<run>:<target>:<round>:<attempt>`) with a `Fingerprint` over the attempt's
input, rather than using one identity for both. A reclaim of the same attempt SHALL re-present
the same `Seal` (witnessed as already performed), while a new round SHALL be a new coordinate and
therefore new work. This mapping SHALL live only in the seam adapters.

#### Scenario: A new round is new work
- **WHEN** a target is addressed again in a later round
- **THEN** the attempt carries a new `Seal` coordinate and shaahid witnesses it as a fresh attempt, not a duplicate

#### Scenario: A reclaimed attempt is not re-performed
- **WHEN** an attempt's settlement is lost and its claim is reclaimed within the same round
- **THEN** shaahid re-presents the same `Seal` and the loop does not re-perform that attempt's side effect

### Requirement: The Round Loop Runs End-To-End Under Agent-Backed Roles
The round loop SHALL be drivable end to end by production, agent-backed Build and Review roles —
not only scripted ones — together with an objective `Verification`, converging as suunta decides.
The Build and Review roles SHALL be supplied through their seams (`RoundBuilder` and
`ReviewRunner`), so the loop depends on no specific agent CLI and each role can be scripted or
agent-backed independently. The goal's satisfaction SHALL remain the `Verification` verdict and
convergence SHALL remain suunta's, regardless of which role implementations are supplied.

#### Scenario: An agent-backed round loop converges
- **WHEN** the round loop is driven by an agent-backed Builder and an agent-backed Reviewer with a verification that passes
- **THEN** the loop converges as suunta decides, having built each round exactly once and honored the Verification verdict as the goal's satisfaction
