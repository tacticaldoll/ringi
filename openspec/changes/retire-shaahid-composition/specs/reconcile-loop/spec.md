## MODIFIED Requirements

### Requirement: The Reconcile Loop Composes The Family Over Public APIs
Ringi SHALL reconcile a desired set of steps to done through a consumer loop that composes the
published suunta 0.1.1 facade (residual planning and convergence) and pacta (durable step
lifecycle) using only the public APIs of those crates. Ringi SHALL add no step-lifecycle state
machine, completion calculation, or idempotency scheme of its own — the only ringi-owned logic is
the loop and thin seam adapters (identity mapping, findings translation). If a seam cannot be
expressed via a brick's public API, that SHALL be recorded as a finding, not worked around by
reaching inside.

#### Scenario: The loop reconciles a desired set to done
- **WHEN** ringi is given a set of desired steps and runs the reconcile loop
- **THEN** it drives each step through pacta's claim/execute/settle, planning with suunta, until every step is done

#### Scenario: No brick behavior is reimplemented
- **WHEN** the reconcile loop needs planning, convergence, or lifecycle
- **THEN** it calls the corresponding brick rather than computing that behavior itself

#### Scenario: Published facade upgrades preserve composition
- **WHEN** ringi resolves suunta from its published 0.1.1 facade crate
- **THEN** the existing convergence, reclaim, restart, and agent-backed composition scenarios remain green without a ringi-owned replacement mechanism

### Requirement: The Composition Is Self-Checked
The reconcile loop SHALL be exercised by a self-checking test that asserts the run converges and
a failed step was withheld then reclaimed, so the bet cannot silently regress under the Definition
of Done.

#### Scenario: A regressed composition fails the gate
- **WHEN** the loop no longer converges or mishandles retry
- **THEN** the self-checking test fails under the Definition of Done

### Requirement: The Round Loop Runs End-To-End Under Agent-Backed Roles
The round loop SHALL be drivable end to end by production, agent-backed Build and Review roles —
not only scripted ones — together with an objective `Verification`, converging as suunta decides.
The Build and Review roles SHALL be supplied through their seams (`RoundBuilder` and
`ReviewRunner`), so the loop depends on no specific agent CLI and each role can be scripted or
agent-backed independently. The goal's satisfaction SHALL remain the `Verification` verdict and
convergence SHALL remain suunta's, regardless of which role implementations are supplied.

#### Scenario: An agent-backed round loop converges
- **WHEN** the round loop is driven by an agent-backed Builder and an agent-backed Reviewer with a verification that passes
- **THEN** the loop converges as suunta decides, having honored the Verification verdict as the goal's satisfaction

## REMOVED Requirements

### Requirement: Each Step Executes Exactly Once
**Reason**: ringi declined shaahid and removed the dependency, so no step attempt is witnessed with
shaahid, and the reconcile loop this described was removed by the dossier pivot.
**Migration**: None. Invocation idempotency is the content-derived invocation coordinate claimed
through the durable pacta registry, specified by `durable-registry` and `resumable-dossiers`.

### Requirement: The Exactly-Once Attempt Identity Is Distinct From The Target Identity
**Reason**: the shaahid `Seal`/`Fingerprint` attempt identity does not exist; ringi declined
shaahid and removed the dependency.
**Migration**: None. The invocation coordinate specified by `resumable-dossiers` identifies an
invocation; no seam adapter maps a shaahid identity.
