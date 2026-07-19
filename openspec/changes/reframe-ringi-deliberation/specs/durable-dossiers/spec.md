## ADDED Requirements

### Requirement: The Durable Store Is Truth For Dossier State
The one SQLite store SHALL persist dossier identity, locked settings, immutable revisions,
append-only events, session references, dissent and condition transitions, human decisions, sealed
evaluation records, and archive integrity metadata. A process restart SHALL reconstruct the current
state without relying on an Agent CLI session.

#### Scenario: A dossier survives process restart
- **WHEN** ringi reopens the store after several committed deliberation turns
- **THEN** it recovers the exact current revision, locked policy, unresolved residual, and event history

### Requirement: Revision And Provenance Commit Atomically
The event that justifies a successor revision and the successor revision SHALL commit in one
transaction. A failed transaction SHALL expose neither record as committed.

#### Scenario: A revision commit is interrupted
- **WHEN** persistence fails while recording an arbitration event and successor revision
- **THEN** the prior revision remains current and no orphan successor or provenance event is visible

### Requirement: Mechanical Facts Outrank NLP Claims
Ringi SHALL derive process exit, timeout, revision linkage, event existence, digest integrity, and
locked-setting enforcement mechanically. No respondent or arbitrator statement SHALL override
those facts.

#### Scenario: An arbitrator cites a missing event
- **WHEN** an arbitration proposal claims to resolve dissent using a provenance identifier absent from the durable event store
- **THEN** ringi rejects the proposal regardless of its natural-language justification
