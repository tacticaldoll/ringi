## REMOVED Requirements

### Requirement: A Step's Work Is Performed By A Builder Agent
**Reason**: the `AgentStepRunner` Builder runner was removed with the reconcile loop.
**Migration**: None. Agent invocation is specified by `agent-adapter`.

### Requirement: A Clean Exit Is Success; Anything Else Retries
**Reason**: the Builder runner this described no longer exists.
**Migration**: `agent-adapter` specifies timeouts and invocation outcomes; `resumable-dossiers`
specifies how an unsettled invocation is retried.

### Requirement: A Round's Build Work Is Performed By A Builder Agent
**Reason**: the `AgentRoundBuilder` was removed with the round loop.
**Migration**: None. Agent invocation is specified by `agent-adapter`.
