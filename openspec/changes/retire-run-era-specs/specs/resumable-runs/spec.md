## REMOVED Requirements

### Requirement: A Run Checkpoints Its Progress Durably
**Reason**: runs no longer exist; the round loop and its checkpoint journal were removed by the
dossier pivot.
**Migration**: Durable progress is the dossier store and the pacta registry; see
`resumable-dossiers` and `durable-registry`.

### Requirement: An Interrupted Run Resumes From Its Recorded Checkpoint
**Reason**: runs no longer exist, so there is no run checkpoint to resume from.
**Migration**: `resumable-dossiers` "A turn resumes after arbitrator failure without re-invoking
the respondent" specifies resumption for dossier turns.

### Requirement: The Resume Command Continues An Interrupted Run
**Reason**: the `ringi resume` command was removed by the dossier pivot.
**Migration**: None. `resumable-dossiers` specifies how an interrupted dossier turn resumes.
