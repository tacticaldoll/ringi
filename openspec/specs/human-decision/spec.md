# human-decision Specification

## Purpose

A human records the terminal disposition of a dossier that has reached `ReadyForDecision`:
approve, reject, cancel, invalidate, or approve with conditions. `approve_with_conditions` is
non-terminal — its conditions must be judged before a dossier can reach plain `Approved`.

## Requirements

### Requirement: An unmet condition is judged by an isolated evaluator invocation

For each condition on a dossier in `ReadyForDecision` whose `is_met` is `false`, the system SHALL
invoke an Agent CLI in the `ConditionEvaluator` role with a prompt containing only the dossier's
latest revision's public SSOT (`original_proposal`, `current_understanding`) and that single
condition's description. The prompt MUST NOT include any other condition, any sealed evaluator
reasoning from a prior evaluation, or any dissent or risk.

#### Scenario: A condition's prompt does not leak another condition's description

- **WHEN** a dossier has two unmet conditions and the first is evaluated
- **THEN** the prompt built for the first condition does not contain the second condition's
  description

### Requirement: Only a True verdict satisfies a condition

A condition's `is_met` SHALL become `true` only when its evaluator invocation's parsed verdict is
`ConditionVerdict::True`. A `False` or `Unknown` verdict SHALL leave `is_met` as `false`.

#### Scenario: A True verdict marks the condition met

- **WHEN** evaluating an unmet condition produces a `ConditionVerdict::True`
- **THEN** that condition's `is_met` becomes `true`

#### Scenario: An Unknown verdict does not mark the condition met

- **WHEN** evaluating an unmet condition produces a `ConditionVerdict::Unknown`
- **THEN** that condition's `is_met` remains `false`

#### Scenario: A False verdict does not mark the condition met

- **WHEN** evaluating an unmet condition produces a `ConditionVerdict::False`
- **THEN** that condition's `is_met` remains `false`

### Requirement: An evaluator's reasoning is sealed

An evaluator's verdict reasoning SHALL be recorded as a `Sealed` event and MUST NOT appear in any
subsequently-built respondent or arbitrator prompt.

#### Scenario: A sealed evaluation event never reaches a respondent prompt

- **WHEN** an evaluator's reasoning is recorded as a sealed event for a dossier
- **AND** a respondent prompt is later built from that dossier's latest revision
- **THEN** the respondent prompt does not contain the sealed reasoning text

### Requirement: An ApprovedWithConditions dossier can be reopened for re-evaluation

A dossier in `ApprovedWithConditions` SHALL be reachable back to `ReadyForDecision` through the
CLI, so its unmet conditions can be judged via `evaluate` and it can eventually reach plain
`Approved`. A dossier not in `ApprovedWithConditions` SHALL be rejected as an invalid transition.

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
