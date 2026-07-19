## ADDED Requirements

### Requirement: Deliberation Advances One Invocation At A Time
Ringi SHALL run at most one respondent, arbitration, or condition-evaluation Agent CLI invocation at
a time for a dossier. It SHALL durably commit the invocation outcome and resulting state before
starting another invocation.

#### Scenario: A response is committed before the next question
- **WHEN** a respondent finishes an answer
- **THEN** ringi records the answer and completes arbitration of that answer before invoking the next respondent

### Requirement: Respondents Answer But Cannot Rewrite The SSOT
A respondent SHALL receive a natural-language question and bounded public dossier context and SHALL
return a natural-language answer. Ringi SHALL record the answer as a claim and SHALL NOT treat the
answer itself as an SSOT revision or a human decision.

#### Scenario: A respondent claims an issue is resolved
- **WHEN** respondent stdout says an unresolved issue is resolved
- **THEN** ringi records the answer but leaves the current revision unchanged until independent arbitration proposes a successor

### Requirement: Independent Arbitration Proposes Complete Successor Revisions
An arbitration session SHALL be distinct from the respondent session whose answer it evaluates. It
SHALL receive the current revision and the new answer and SHALL propose a complete successor
revision containing current understanding, positions, dissent, unresolved risks, readiness, and a
next question when more deliberation is required.

#### Scenario: Arbitration integrates one answer
- **WHEN** a respondent answer has been recorded
- **THEN** an independent arbitration invocation proposes a complete successor to the current dossier revision

### Requirement: Unresolved Dissent Is Conservatively Retained
An unresolved dissent SHALL remain unresolved in a successor revision unless arbitration supplies a
resolution reason and references the recorded answer events supporting that resolution. Resolution
SHALL remain in history, and later evidence MAY reopen it with new provenance.

#### Scenario: Unsupported dissent removal fails closed
- **WHEN** an arbitration proposal omits an existing unresolved dissent without a resolution reason and valid provenance references
- **THEN** ringi rejects the successor revision and retains the current revision

#### Scenario: Later evidence reopens a resolution
- **WHEN** arbitration cites a later answer that invalidates an earlier resolution
- **THEN** the successor revision marks the dissent unresolved again while preserving both transitions

### Requirement: Decision Readiness Never Grants Approval
Arbitration MAY report that a dossier is ready for human decision when its configured readiness
criteria are met. Ringi SHALL stop automatic questioning at that point, but only a human decision
event SHALL approve, reject, condition, cancel, or invalidate the dossier.

#### Scenario: A ready dossier waits for a human
- **WHEN** arbitration reports that no blocking question or risk remains
- **THEN** ringi marks the dossier ready for decision without marking it approved
