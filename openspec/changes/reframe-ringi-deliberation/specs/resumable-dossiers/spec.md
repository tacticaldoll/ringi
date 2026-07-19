## ADDED Requirements

### Requirement: A Dossier Resumes Without Hidden Session State
Ringi SHALL continue a non-terminal submitted dossier from its latest committed revision and event
history. If a persistent Agent session is unavailable, the locked strategy SHALL use or fall back to
a permitted fresh session without changing durable dossier meaning.

#### Scenario: Ringi restarts during deliberation
- **WHEN** a process restarts after committing a respondent answer but before starting arbitration
- **THEN** it resumes by arbitrating that recorded answer exactly once against the same input revision

### Requirement: Invocation Identity Is Stable Across Recovery
One invocation SHALL be identified by dossier, role, fixed input revision or condition evaluation
snapshot, turn, and attempt. Reclaiming that coordinate SHALL reuse a witnessed answer; changing its
input under the same coordinate SHALL fail as an identity contradiction.

#### Scenario: Recovery reclaims a completed invocation
- **WHEN** a process loses settlement after recording an Agent answer and later reclaims the same invocation coordinate
- **THEN** ringi attaches the witnessed answer without invoking the Agent CLI again

### Requirement: Terminal Dossiers Cannot Resume
Approved, rejected, cancelled, and invalidated dossiers SHALL be terminal and SHALL reject further
automatic turns or human decisions.

#### Scenario: A user continues an archived dossier
- **WHEN** a user requests continuation of a terminal dossier
- **THEN** ringi rejects the request and leaves the archive unchanged
