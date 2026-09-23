## REMOVED Requirements

### Requirement: Ringi Objectively Certifies The Goal Through A Verification Seam
**Reason**: the `Verification` seam (`verify.rs`) was removed by the dossier pivot.
**Migration**: None. Dossier convergence is specified by `deliberation-loop`.

### Requirement: Verification Runs Config-Supplied Commands Objectively
**Reason**: no verification commands are configured or run.
**Migration**: None. Spawning without a shell and under a timeout is specified for agent
invocation by `agent-adapter`.

### Requirement: Tool Verification Outranks Model Opinion
**Reason**: no tool verification exists to outrank model opinion.
**Migration**: `deliberation-loop` "The arbitrator does not output or store readiness" keeps model
output from deciding readiness.
