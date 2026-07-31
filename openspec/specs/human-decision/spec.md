# human-decision Specification

## Purpose

A human records the terminal disposition of a dossier that has reached `ReadyForDecision`:
approve, reject, cancel, invalidate, or approve with conditions. `approve_with_conditions` is
non-terminal — its conditions must be judged before a dossier can reach plain `Approved`.

## Requirements

### Requirement: A condition is a revision-level residual item, mutated only through ConditionMove

A `Condition` SHALL be a field of `Revision` (stable id, description, and an optional
`Resolution` carrying the reason and event provenance that satisfied it), not of the dossier.
Every mutation — adding a new condition, or satisfying an existing one — SHALL go through a
`ConditionMove` (`Add`/`Satisfy`) applied via the same `residual_ledger` seam that composes
`cadw`'s `Ledger` for dissents, risks, and questions, producing a new successor revision. The
arbitrator's `Move` enum SHALL NOT gain any condition-related variant — a condition can never be
added or satisfied by an arbitrator invocation, only by `add_condition_command` (human-authored)
or `evaluate_conditions` (evaluator-judged).

#### Scenario: Adding a condition produces a successor revision

- **WHEN** a human adds a condition to a dossier in `ReadyForDecision`
- **THEN** a new revision is committed whose `conditions` includes the new condition, unresolved

#### Scenario: A satisfied condition carries the reason and provenance that satisfied it

- **WHEN** a condition is satisfied by a `True` verdict
- **THEN** the successor revision's matching condition has a `Resolution` with that verdict's
  reason and event provenance, reloadable after a fresh store connection

### Requirement: An unmet condition is judged by an isolated evaluator invocation

For each condition on the dossier's latest revision whose `resolved_by` is `None`, the system
SHALL invoke an Agent CLI in the `ConditionEvaluator` role with a prompt containing only the
dossier's latest revision's public SSOT (`original_proposal`, `current_understanding`) and that
single condition's description. The prompt MUST NOT include any other condition, any sealed
evaluator reasoning from a prior evaluation, or any dissent, risk, or question.

#### Scenario: A condition's prompt does not leak another condition's description

- **WHEN** a dossier has two unmet conditions and the first is evaluated
- **THEN** the prompt built for the first condition does not contain the second condition's
  description

### Requirement: Only a True verdict satisfies a condition

A condition's `resolved_by` SHALL become `Some` only when its evaluator invocation's parsed
verdict is `ConditionVerdict::True`, applying `ConditionMove::Satisfy` with that verdict's reason
and provenance. A `False` or `Unknown` verdict SHALL apply no move at all — the condition's
`resolved_by` stays `None`, exactly as an unresolved risk or question is left untouched by a
`Move` batch that doesn't address it — and SHALL release the evaluation's claim for retry under
the same coordinate, rather than settling it fulfilled: a negative or uncertain verdict is a
normal, expected answer that later circumstances may change, not a permanent fact about the
condition.

#### Scenario: A True verdict satisfies the condition

- **WHEN** evaluating an unmet condition produces a `ConditionVerdict::True`
- **THEN** that condition's `resolved_by` becomes `Some`, carrying the verdict's reason and
  provenance

#### Scenario: An Unknown verdict does not satisfy the condition

- **WHEN** evaluating an unmet condition produces a `ConditionVerdict::Unknown`
- **THEN** that condition's `resolved_by` remains `None`

#### Scenario: A False verdict does not satisfy the condition

- **WHEN** evaluating an unmet condition produces a `ConditionVerdict::False`
- **THEN** that condition's `resolved_by` remains `None`

#### Scenario: A negative verdict's coordinate remains claimable for retry

- **WHEN** a condition evaluator returns a `False` or `Unknown` verdict
- **THEN** the same coordinate remains claimable — a later `evaluate` call (after the dossier is
  reopened and the underlying circumstance changes) re-invokes the evaluator instead of failing
  with "already settled"

#### Scenario: A prior negative verdict does not block a later evaluate call from reaching a subsequent condition

- **WHEN** one condition already evaluated to `False` or `Unknown` in an earlier `evaluate` call,
  and a later `evaluate` call reaches it again before any later, still-unattempted condition
- **THEN** the later call still reaches and evaluates the subsequent condition, rather than
  failing on the earlier one and never getting there

### Requirement: An evaluator's reasoning is sealed

An evaluator's verdict reasoning SHALL be recorded as a `Sealed` event and MUST NOT appear in any
subsequently-built respondent or arbitrator prompt.

#### Scenario: A sealed evaluation event never reaches a respondent prompt

- **WHEN** an evaluator's reasoning is recorded as a sealed event for a dossier
- **AND** a respondent prompt is later built from that dossier's latest revision
- **THEN** the respondent prompt does not contain the sealed reasoning text

### Requirement: An ApprovedWithConditions dossier can be reopened for re-evaluation

A dossier in `ApprovedWithConditions` SHALL be reachable back to `ReadyForDecision` through the
CLI, so its unmet conditions (on its latest revision) can be judged via `evaluate` and it can
eventually reach plain `Approved`. A dossier not in `ApprovedWithConditions` SHALL be rejected as
an invalid transition.

#### Scenario: Reopening an ApprovedWithConditions dossier reaches ReadyForDecision

- **WHEN** a dossier in `ApprovedWithConditions` is reopened
- **THEN** its state becomes `ReadyForDecision`

#### Scenario: Reopening a dossier not in ApprovedWithConditions is rejected

- **WHEN** a dossier not in `ApprovedWithConditions` is reopened
- **THEN** the transition is rejected as invalid and the dossier's state does not change

#### Scenario: A reopened dossier's conditions can be evaluated again

- **WHEN** an `ApprovedWithConditions` dossier with an unmet condition is reopened and then
  evaluated
- **THEN** the evaluation runs (it is no longer refused for being outside `ReadyForDecision`)
