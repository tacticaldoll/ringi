## REMOVED Requirements

### Requirement: A Run's State Is Persisted To The One Durable Store
**Reason**: runs no longer exist; the store now holds dossiers.
**Migration**: `dossier-cli` "Dossier state and registry state share one store" carries the
shared-store guarantee for dossiers; `deliberation-dossier` specifies what a dossier persists.

### Requirement: Init Provisions The Durable Store
**Reason**: the requirement is phrased for runs and `ringi run`, which no longer exist.
**Migration**: `dossier-cli` "Init provisions the durable store without destroying existing data"
restates it for dossiers.

### Requirement: Status Reads A Persisted Run
**Reason**: the `ringi status` command was removed by the dossier pivot.
**Migration**: None. `ringi inspect` reads a dossier; see `dossier-cli`.
