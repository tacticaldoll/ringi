## REMOVED Requirements

### Requirement: The Reconcile Loop Composes The Family Over Public APIs
**Reason**: the reconcile/round loop (`reconcile.rs`, `run.rs`) was removed by the dossier pivot; no step set is reconciled any more.
**Migration**: Dossier deliberation composes suunta and pacta instead; see `deliberation-loop`,
`resumable-dossiers`, and `durable-registry`.

### Requirement: Convergence Is Decided By Suunta
**Reason**: the reconcile/round loop (`reconcile.rs`, `run.rs`) was removed by the dossier pivot.
**Migration**: `deliberation-loop` "Convergence is computed mechanically by suunta" specifies
suunta-decided convergence for dossiers.

### Requirement: A Failed Step Retries Via Deferred Reclaim
**Reason**: the reconcile/round loop (`reconcile.rs`, `run.rs`) was removed by the dossier pivot; there are no steps to retry.
**Migration**: Releasing a claimed invocation for retry is specified by `resumable-dossiers` and
`durable-registry`.

### Requirement: The Composition Is Self-Checked
**Reason**: the reconcile/round loop (`reconcile.rs`, `run.rs`) was removed by the dossier pivot, together with its self-checking composition test.
**Migration**: None. The Definition of Done exercises the surviving dossier capabilities through
their own tests.

### Requirement: Step Execution Is Delegated To A Runner Seam
**Reason**: the reconcile/round loop (`reconcile.rs`, `run.rs`) was removed by the dossier pivot; the `StepRunner` seam no longer exists.
**Migration**: None. Agent invocation goes through the seam specified by `agent-adapter`.

### Requirement: The Loop Reconciles A Changing Bearing Of Goal Plus Findings
**Reason**: the reconcile/round loop (`reconcile.rs`, `run.rs`) was removed by the dossier pivot; no goal-plus-findings Bearing is built.
**Migration**: `deliberation-loop` and `deliberation-dossier` specify the dossier's targets
(dissents, risks, questions) and how suunta converges over them.

### Requirement: Two Certifiers Feed The Sounding
**Reason**: the reconcile/round loop (`reconcile.rs`, `run.rs`) was removed by the dossier pivot, together with the `Verification` and Reviewer certifiers.
**Migration**: None. `deliberation-loop` specifies how each dossier target carries an explicit
finding.

### Requirement: The Round Loop Runs End-To-End Under Agent-Backed Roles
**Reason**: the reconcile/round loop (`reconcile.rs`, `run.rs`) was removed by the dossier pivot; the agent-backed Build and Review roles were removed with it.
**Migration**: None. Agent-backed dossier turns are specified by `deliberation-loop` and
`resumable-dossiers`.
