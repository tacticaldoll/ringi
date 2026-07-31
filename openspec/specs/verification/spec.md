# verification Specification

## Purpose

How ringi decides the goal is met: an objective `Verification` verdict that ringi certifies
itself (re-running checks), never an agent's claim. Tool verification outranks model opinion —
the goal is not done until Verify returns green, regardless of Reviewer output. The production
verdict comes from running config-supplied commands (build/test/lint); scripted implementations
exist for tests.

## Requirements

### Requirement: Ringi Objectively Certifies The Goal Through A Verification Seam
Ringi SHALL certify whether the goal is met through a `Verification` seam that returns an
objective pass/fail verdict, and the loop SHALL take that verdict — never an agent's claim — as
the goal's satisfaction. The seam SHALL be ringi's own (it re-runs objective checks). Its
production implementation SHALL execute real verification commands (see "Verification Runs
Config-Supplied Commands Objectively"); scripted implementations MAY exist for tests. The
verdict SHALL map to the goal target's suunta `Satisfaction`.

#### Scenario: A green verdict satisfies the goal
- **WHEN** the `Verification` seam returns pass for the current state
- **THEN** the loop records the goal target as Satisfied for that cycle

#### Scenario: A red verdict keeps the goal unsatisfied
- **WHEN** the `Verification` seam returns fail
- **THEN** the goal target stays Unsatisfied and remains in the residual for another round

### Requirement: Verification Runs Config-Supplied Commands Objectively
The production `Verification` SHALL certify the goal by running a set of config-supplied
verification commands and computing the verdict itself. Each command SHALL be supplied as a
program and an argument vector and SHALL be spawned directly as program + arguments, never
through a shell, in the run's workspace, and bounded by a timeout. The verdict SHALL be pass if
and only if every configured command completes with a zero exit status; any non-zero exit, a
failure to spawn, or a timeout SHALL yield a fail verdict. No command output and no agent output
SHALL set or override the verdict. Command execution SHALL compose the same hardened subprocess
mechanism used to invoke agents rather than a second, independent spawn path.

#### Scenario: All commands pass yields a green verdict
- **WHEN** every configured verification command exits with status zero
- **THEN** the seam returns pass and the goal target is recorded Satisfied for that cycle

#### Scenario: Any failing command yields a red verdict
- **WHEN** at least one configured verification command exits non-zero
- **THEN** the seam returns fail and the goal target stays Unsatisfied

#### Scenario: A spawn failure or timeout is a red verdict, not an error to the loop
- **WHEN** a configured command cannot be spawned or does not finish within its timeout
- **THEN** the seam returns fail and the loop drives another round rather than aborting

#### Scenario: Commands are spawned without a shell
- **WHEN** a verification command is executed
- **THEN** it is spawned as program plus argument vector directly, never via a shell, so no shell
  interpretation or argument injection can occur

### Requirement: Tool Verification Outranks Model Opinion
The goal's completion SHALL be decided solely by the `Verification` verdict, and SHALL NOT be
inferred from the absence of Reviewer findings or from any agent's assertion of success. A run
SHALL NOT be reported complete while verification has not returned a green verdict, regardless of
Reviewer output.

#### Scenario: No findings does not imply done
- **WHEN** the Reviewer returns no findings but verification has not returned green
- **THEN** the goal is not satisfied and the run does not converge
