## REMOVED Requirements

### Requirement: The Run Command Drives A Run And Presents Its Outcome
**Reason**: Ringi no longer drives a workspace-editing code run.
**Migration**: Use the new dossier lifecycle commands to draft, submit, continue, inspect, and decide one deliberation.

### Requirement: The Config File Supplies Run Parameters
**Reason**: Builder, workspace, and command-verification parameters are removed.
**Migration**: Configure Agent roles, arbitration strategy, and limits in dossier frontmatter and project defaults.

### Requirement: The Init Command Scaffolds A Config File
**Reason**: The old scaffold describes the removed code-execution flow.
**Migration**: Use the deliberation CLI initialization and draft surfaces specified by `deliberation-dossier` and `arbitration-strategy`.
