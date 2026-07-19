## ADDED Requirements

### Requirement: Only A Human Makes A Dossier Decision
Ringi SHALL accept human decisions to approve, reject, approve with conditions, cancel, or
invalidate. No Agent answer, arbitration verdict, confidence value, or completed condition SHALL
create a final approval.

#### Scenario: Arbitration recommends approval
- **WHEN** an arbitrator recommends approval and reports high confidence
- **THEN** the dossier remains ready for decision until a human records a decision

### Requirement: Conditional Approval Adds Fixed Predicates
Approve-with-conditions SHALL be non-terminal. It SHALL add human-authored, immutable condition
predicates to the dossier residual and return the dossier to deliberation. Agents SHALL NOT modify,
dismiss, or accept the risk of a condition.

#### Scenario: A human adds conditions
- **WHEN** a human conditionally approves a ready dossier with conditions C1 and C2
- **THEN** ringi records the decision event, locks C1 and C2, and resumes deliberation with both conditions unresolved

### Requirement: Conditions Are Evaluated In Isolated Sessions
Each condition evaluation SHALL use a session isolated from the respondent answer it evaluates and
SHALL produce `true`, `false`, or `unknown`, a concise justification, and provenance references.
Ringi SHALL use only the verdict for control flow and SHALL classify the justification as sealed
audit material.

#### Scenario: A condition lacks sufficient support
- **WHEN** the evaluator cannot establish a condition from the current answer and evidence
- **THEN** it returns unknown and the condition remains unresolved

### Requirement: Evaluator Feedback Never Coaches Respondents
Ringi SHALL mechanically exclude condition-evaluation justifications and evaluator prompts from
every respondent and synthesis context. Respondents MAY see the fixed condition and public SSOT but
SHALL NOT receive evaluator reasoning.

#### Scenario: A false condition starts another answer turn
- **WHEN** a condition evaluator returns false with a detailed justification
- **THEN** ringi archives that justification but constructs the next respondent prompt without it

### Requirement: Fulfilled Conditions Return To Human Decision
When every fixed condition is true, ringi SHALL mark the dossier ready for another human decision.
It SHALL NOT convert the earlier conditional approval into a final approval automatically.

#### Scenario: All conditions become true
- **WHEN** the final unresolved condition receives a true verdict
- **THEN** ringi presents the dossier for a new human decision and leaves final status unapproved

### Requirement: Invalidation Preserves A Failed Proceeding
If a human determines that arbitration is untrustworthy, ringi SHALL terminate the dossier as
invalidated, preserve the invalidation reason and complete history, and SHALL NOT overwrite or
reinterpret the disputed verdict.

#### Scenario: A human disputes arbitration
- **WHEN** a human invalidates a dossier after inspecting sealed arbitration reasons
- **THEN** ringi stops the dossier and archives it as invalidated rather than overriding the verdict
