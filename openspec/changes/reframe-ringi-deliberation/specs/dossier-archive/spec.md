## ADDED Requirements

### Requirement: Every Terminal Dossier Produces An Immutable Human-Readable Archive
Ringi SHALL archive approved, rejected, cancelled, and invalidated dossiers. The archive SHALL
contain the original proposal, final public SSOT, locked strategy and limits, revision and event
index, decisions, provenance, and integrity digests in a human-readable form.

#### Scenario: An approved dossier is archived
- **WHEN** a human finally approves a dossier
- **THEN** ringi emits an immutable archive that identifies the exact proposal and revision approved

### Requirement: Sealed Arbitration Records Are Archived Separately
The archive SHALL include arbitration and condition-evaluation verdicts, concise justifications,
evidence references, session identity, and model or adapter metadata in a clearly marked sealed
section. Sealed content SHALL NOT be part of any respondent-context projection.

#### Scenario: A human audits an automatic resolution
- **WHEN** a human inspects an archived automatically resolved dissent
- **THEN** the archive shows the verdict, justification, cited events, and arbitrator provenance without having exposed them to respondents

### Requirement: Archive Integrity Is Verifiable
Every archived revision and decision SHALL be bound to deterministic content digests that exclude
their own digest field. Ringi SHALL detect a changed archived body, frontmatter setting, parent
link, sealed record, or decision as an integrity failure.

#### Scenario: Archived content is modified
- **WHEN** archive verification recomputes a digest after any bound content has changed
- **THEN** verification fails and identifies the affected record

### Requirement: Approval Has No Built-In Execution Effect
An approved archive SHALL remain a deliberation record. Ringi SHALL NOT edit a workspace, invoke a
downstream executor, apply a patch, or grant execution authority as a consequence of approval.

#### Scenario: A dossier is approved
- **WHEN** a human records final approval
- **THEN** ringi archives the decision and performs no workspace or external execution action
