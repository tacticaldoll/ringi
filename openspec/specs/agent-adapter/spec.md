# agent-adapter Specification

## Purpose

The seam by which ringi drives heterogeneous Agent CLIs — a uniform adapter over a subprocess,
so the orchestration never depends on any one CLI's flags. The transport reports outcomes
(exit code, captured output, best-effort structured output); it does not judge them. Its one
non-negotiable invariant is that an agent is spawned as a program with arguments, never
through a shell.

## Requirements

### Requirement: Ringi Invokes Agents Through A Uniform Adapter
Ringi SHALL invoke Agent CLIs through an `AgentAdapter` seam so callers never depend on a
specific CLI's flags. A `SubprocessAdapter` SHALL run a configured agent as a subprocess in
the run's workspace directory and return an `AgentResponse` carrying the exit code, captured
stdout and stderr, and the agent's parsed structured output. Adapter selection and CLI flags
SHALL be the adapter's concern, not the caller's.

#### Scenario: An agent runs and its response is returned
- **WHEN** ringi runs an agent through a `SubprocessAdapter` with a prompt and workspace
- **THEN** it returns an `AgentResponse` with the exit code, stdout, stderr, and any structured output

### Requirement: Agent Invocation Uses Program And Arguments, Never A Shell
The adapter SHALL spawn the agent as a program with an argument list, never by passing a
constructed string to a shell, so agent- or task-derived text can never become an executed
shell command.

#### Scenario: No shell interpretation
- **WHEN** the adapter spawns an agent
- **THEN** it invokes the program with explicit arguments and does not route the invocation through a shell

### Requirement: Agent Invocation Is Bounded By A Timeout
Every agent invocation SHALL be bounded by a caller-supplied timeout; on expiry the adapter
SHALL terminate the child process and return a timeout error rather than block indefinitely.

#### Scenario: A hung agent is terminated
- **WHEN** an agent does not exit within its timeout
- **THEN** the adapter kills the child and returns a timeout error

### Requirement: Structured Output Is Parsed Best-Effort
The adapter SHALL parse the agent's structured output from its stdout as JSON, retaining the
raw stdout regardless. A missing or malformed structure SHALL NOT be an error — it yields no
structured value — and a non-zero exit SHALL be reported in the response, not raised;
infrastructure failures (spawn failure, timeout) are the adapter's error cases.

#### Scenario: Malformed structured output is not an infrastructure error
- **WHEN** an agent exits but its stdout has no valid trailing JSON object
- **THEN** the response carries the raw stdout with no structured value, and no error is raised

#### Scenario: A non-zero exit is reported, not raised
- **WHEN** an agent exits non-zero
- **THEN** the exit code is reported in the response for the caller to judge, not raised as an adapter error
