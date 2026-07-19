## ADDED Requirements

### Requirement: The MVP Offers Three Inspectable Arbitration Strategies
Ringi SHALL offer economy, balanced, and assurance presets. Each preset SHALL resolve to explicit,
inspectable session scope, fresh-review triggers, thresholds, and limits before dossier submission.

#### Scenario: A user selects a cost-quality posture
- **WHEN** a user selects economy, balanced, or assurance for a draft
- **THEN** ringi shows and stores the explicit settings that the preset will lock at submission

### Requirement: Economy Uses Persistent Arbitration
The economy strategy SHALL use one persistent arbitration session for ordinary synthesis and
automatic dissent resolution until that session fails or the dossier terminates.

#### Scenario: Economy advances multiple rounds
- **WHEN** an economy dossier completes successive respondent turns
- **THEN** ringi resumes the same arbitration session while still supplying the current durable revision each turn

### Requirement: Balanced Escalates Selected Judgments To Fresh Review
The balanced strategy SHALL use persistent arbitration for ordinary turns and SHALL use a fresh
arbitration session when any locked low-confidence, severity, or pre-decision trigger matches.

#### Scenario: A pre-decision trigger opens fresh review
- **WHEN** persistent arbitration reports decision readiness under a balanced strategy configured for pre-decision review
- **THEN** ringi obtains the required fresh-session review before presenting the dossier for human decision

### Requirement: Assurance Uses Fresh Arbitration At Its Locked Granularity
The assurance strategy SHALL create fresh arbitration sessions at the locked per-round or
per-resolution granularity and SHALL NOT silently fall back to a persistent session.

#### Scenario: Assurance independently evaluates a resolution
- **WHEN** an assurance dossier reaches an automatic dissent resolution boundary configured as fresh-per-resolution
- **THEN** ringi starts a fresh arbitration session using only the durable input revision and cited events

### Requirement: Session Memory Is Never The Source Of Truth
Every strategy SHALL supply the current durable revision to arbitration and SHALL be able to
continue with a fresh session after process loss. Persistent session state MAY reduce cost but SHALL
NOT be required to reconstruct dossier state.

#### Scenario: A persistent session cannot be resumed
- **WHEN** ringi recovers a dossier but the recorded arbitration session is unavailable
- **THEN** it starts a fresh session from the current revision without losing committed dossier state
