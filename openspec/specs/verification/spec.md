# verification Specification

## Purpose

How ringi decides the goal is met: an objective `Verification` verdict that ringi certifies
itself (re-running checks), never an agent's claim. Tool verification outranks model opinion —
the goal is not done until Verify returns green, regardless of Reviewer output. This increment's
implementation is scripted; real command execution (build/test/lint) lands later.

## Requirements

### Requirement: Ringi Objectively Certifies The Goal Through A Verification Seam
Ringi SHALL certify whether the goal is met through a `Verification` seam that returns an
objective pass/fail verdict, and the loop SHALL take that verdict — never an agent's claim — as
the goal's satisfaction. The seam SHALL be ringi's own (it re-runs objective checks); in this
increment its implementation MAY be scripted, with real command execution (build/test/lint)
deferred to a later change. The verdict SHALL map to the goal target's suunta `Satisfaction`.

#### Scenario: A green verdict satisfies the goal
- **WHEN** the `Verification` seam returns pass for the current state
- **THEN** the loop records the goal target as Satisfied for that cycle

#### Scenario: A red verdict keeps the goal unsatisfied
- **WHEN** the `Verification` seam returns fail
- **THEN** the goal target stays Unsatisfied and remains in the residual for another round

### Requirement: Tool Verification Outranks Model Opinion
The goal's completion SHALL be decided solely by the `Verification` verdict, and SHALL NOT be
inferred from the absence of Reviewer findings or from any agent's assertion of success. A run
SHALL NOT be reported complete while verification has not returned a green verdict, regardless of
Reviewer output.

#### Scenario: No findings does not imply done
- **WHEN** the Reviewer returns no findings but verification has not returned green
- **THEN** the goal is not satisfied and the run does not converge
