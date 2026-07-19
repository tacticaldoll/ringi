## REMOVED Requirements

### Requirement: A Run's State Is Persisted To The One Durable Store
**Reason**: Workspace code runs are no longer ringi's domain.
**Migration**: Persist dossiers, revisions, invocation events, dissent transitions, conditions, sealed evaluations, and decisions in the one durable store.

### Requirement: Init Provisions The Durable Store
**Reason**: The existing schema provisions removed run records.
**Migration**: Initialization provisions the dossier schema without destroying existing dossier data.

### Requirement: Status Reads A Persisted Run
**Reason**: The CLI no longer exposes code-run status.
**Migration**: Dossier inspection reads the current revision, lifecycle state, readiness, and archive from the durable store.
