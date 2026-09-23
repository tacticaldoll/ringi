## REMOVED Requirements

### Requirement: The Run Command Drives A Run And Presents Its Outcome
**Reason**: the `ringi run` command was removed by the dossier pivot.
**Migration**: None. The dossier commands are specified by `dossier-cli`.

### Requirement: The Config File Supplies Run Parameters
**Reason**: the TOML run config (`config.rs`) was removed by the dossier pivot.
**Migration**: None. No command reads a run config file.

### Requirement: The Init Command Scaffolds A Config File
**Reason**: `ringi init` no longer writes a config file.
**Migration**: `dossier-cli` "Init provisions the durable store without destroying existing data"
specifies what `ringi init` does.
