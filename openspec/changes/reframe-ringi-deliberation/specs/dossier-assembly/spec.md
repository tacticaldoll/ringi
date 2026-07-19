## ADDED Requirements

### Requirement: Ringi Assembles Deliberation Roles From Locked Settings
Ringi SHALL assemble respondent, arbitration, and condition-evaluation Agent adapters plus the
resolved strategy and limits recorded at dossier submission. Assembly SHALL NOT add workspace
editing, command verification, or provider-layer behavior.

#### Scenario: A submitted dossier starts deliberation
- **WHEN** ringi continues a submitted dossier
- **THEN** it constructs the configured roles and exact locked strategy from durable frontmatter

### Requirement: Role Sessions Remain Logically Separate
Respondent, arbitration, and evaluator roles SHALL NOT share one conversational session. Persistent
arbitration MAY span arbitration turns but SHALL remain separate from every respondent and evaluator
session.

#### Scenario: One CLI backs multiple roles
- **WHEN** the same Agent CLI program is configured for respondent and arbitration roles
- **THEN** ringi invokes them with distinct session identities and role-specific bounded context

### Requirement: Deliberation Mechanics Are Composed Without Reimplementation
The synchronous dossier loop SHALL use public family contracts only where they own a required
mechanic. Ringi SHALL own role prompts, dossier projections, domain identity, provenance, and human
decision wiring, and SHALL NOT recreate lifecycle, convergence, or idempotency schemes already
provided by retained family dependencies.

#### Scenario: The composition is adversarially reviewed
- **WHEN** the new dossier loop is complete
- **THEN** its self-check proves recovery and residual behavior while the apply review finds no ringi-owned duplicate family engine
