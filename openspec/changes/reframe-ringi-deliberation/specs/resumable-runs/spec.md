## REMOVED Requirements

### Requirement: A Run Checkpoints Its Progress Durably
**Reason**: Build-round checkpoints are removed.
**Migration**: Atomic dossier revisions and invocation events are the deliberation checkpoints.

### Requirement: An Interrupted Run Is Resumed From Its Checkpoint
**Reason**: Ringi no longer resumes code execution rounds.
**Migration**: Resume from the current committed dossier revision and completed invocation identity.

### Requirement: The Resume Command Continues An Interrupted Run
**Reason**: The old command reconstructs workspace task configuration.
**Migration**: Continue a non-terminal submitted dossier from durable state and locked frontmatter.
