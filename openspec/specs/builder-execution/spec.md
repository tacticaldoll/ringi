# builder-execution Specification

## Purpose

How ringi performs a step's actual work: by running a Builder agent through the agent seam. The
loop delegates a step's work to a runner (see `reconcile-loop`); the Builder runner is the
production runner, mapping an agent's result to a step outcome — a clean exit is success,
anything else is a failure the loop retries. It adds no retry or backoff of its own.

## Requirements

### Requirement: A Step's Work Is Performed By A Builder Agent
Ringi SHALL perform a step's work by running a Builder agent through the agent seam: an
`AgentStepRunner` SHALL turn a step into a Builder `AgentRequest` (in the run's workspace,
bounded by a timeout) and run it through an `AgentAdapter`. The runner SHALL depend only on the
`AgentAdapter` seam, never on a specific CLI, so any adapter can back it.

#### Scenario: A step runs its Builder agent
- **WHEN** the loop executes a step through an `AgentStepRunner`
- **THEN** the runner invokes the configured Builder agent through the adapter in the workspace and returns the step's outcome

### Requirement: A Clean Exit Is Success; Anything Else Retries
The runner SHALL treat an agent that exits cleanly (a zero exit code) as a successful step, and
SHALL treat every other result — a non-zero exit, a spawn failure, or a timeout — as a failed
step, so the loop retries it through pacta's deferred reclaim. The runner SHALL NOT compute
backoff or retry itself; it only reports success or failure.

#### Scenario: A clean exit succeeds the step
- **WHEN** the Builder agent exits with a zero exit code
- **THEN** the runner reports success and the loop settles the step

#### Scenario: A non-zero exit fails the step
- **WHEN** the Builder agent exits non-zero, fails to spawn, or times out
- **THEN** the runner reports failure and the loop retries the step via deferred reclaim
