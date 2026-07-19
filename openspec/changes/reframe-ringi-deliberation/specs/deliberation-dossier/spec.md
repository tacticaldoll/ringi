## ADDED Requirements

### Requirement: A Dossier Is The Unit Of Deliberation
Ringi SHALL automate one dossier from a human-authored draft through one terminal human decision.
The dossier body SHALL contain the original proposal, current understanding, positions, dissent,
unresolved risks, and decision readiness in human-readable natural language.

#### Scenario: A user creates a dossier draft
- **WHEN** a user starts a new deliberation with a proposal
- **THEN** ringi creates one draft dossier whose body is the working SSOT for that deliberation

### Requirement: Submission Locks User-Controlled Settings
The dossier frontmatter SHALL contain its identity, arbitration strategy, limits, and Agent role
bindings. A user MAY edit those fields while the dossier is a draft. Submission SHALL resolve all
presets to explicit settings and SHALL make every user-controlled setting immutable for that
dossier.

#### Scenario: A submitted strategy cannot change
- **WHEN** a user attempts to change a strategy or limit after submitting a dossier
- **THEN** ringi rejects the change without modifying the submitted dossier

### Requirement: Every SSOT Update Is An Immutable Successor Revision
Ringi SHALL commit each complete successor body as a new immutable revision with a parent digest.
Only ringi-owned lifecycle and provenance fields MAY differ across submitted revisions. The current
revision SHALL be the sole working truth supplied to a later respondent.

#### Scenario: Arbitration advances the SSOT
- **WHEN** ringi accepts an arbitration proposal based on revision N
- **THEN** it atomically records revision N+1 with N's digest as its parent and makes N+1 current

### Requirement: Raw History Does Not Become Respondent Context
Ringi SHALL preserve prompts, answers, arbitration proposals, verdicts, and decisions as append-only
events. It SHALL construct respondent context from the original proposal, current public revision,
and current unresolved items, without replaying the raw event transcript or sealed evaluation
records.

#### Scenario: A later respondent receives bounded context
- **WHEN** ringi invokes a respondent after multiple completed turns
- **THEN** the prompt contains the current dossier revision but excludes prior raw transcripts and sealed evaluation reasons
