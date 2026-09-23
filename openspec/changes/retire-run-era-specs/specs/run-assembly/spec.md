## REMOVED Requirements

### Requirement: Ringi Assembles A Run From Its Configuration
**Reason**: the run composition root (`run_from_config`, `RunConfig`) was removed by the dossier
pivot.
**Migration**: None. Dossier commands wire their own dependencies; see `dossier-cli`.
