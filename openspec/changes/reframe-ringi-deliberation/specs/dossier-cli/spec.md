## ADDED Requirements

### Requirement: The CLI Drives One Dossier Lifecycle
The CLI SHALL let a user create and edit a draft, submit its locked settings, continue automatic
deliberation, inspect public and sealed records, and record a human decision. Commands SHALL use
deliberative vocabulary and SHALL NOT require a workspace or code task.

#### Scenario: A user completes one dossier
- **WHEN** a user drafts, submits, continues, and finally decides a dossier
- **THEN** the CLI advances the same durable dossier and reports its immutable archive location

### Requirement: The CLI Fails Closed On Invalid State Transitions
The CLI SHALL reject attempts to change locked settings, continue terminal dossiers, decide dossiers
that are not awaiting human input, or expose sealed evaluator material as respondent context.

#### Scenario: A user tries to edit a submitted strategy
- **WHEN** a CLI command requests a strategy change for a submitted dossier
- **THEN** the command exits non-zero and leaves the dossier unchanged
