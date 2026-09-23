## REMOVED Requirements

### Requirement: A Reviewer Agent Produces Findings Through The Agent Seam
**Reason**: the Reviewer runner was removed with the round loop.
**Migration**: None. Agent invocation is specified by `agent-adapter`; dossier findings are
specified by `deliberation-loop`.

### Requirement: Reviewer Output Is A Quality Opinion, Never A Permission
**Reason**: the Reviewer role no longer exists.
**Migration**: `deliberation-loop` "The arbitrator does not output or store readiness" keeps model
output from deciding readiness; `human-decision` keeps the decision with the human.
