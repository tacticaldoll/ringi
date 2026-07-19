## MODIFIED Requirements

### Requirement: Ringi Invokes Agents Through A Uniform Adapter
Ringi SHALL invoke every respondent, arbitrator, and condition evaluator through a uniform
`AgentAdapter`. A request SHALL carry the natural-language prompt, role, session instruction,
working directory, explicit environment, and timeout. A response SHALL carry process outcome,
stdout as an opaque natural-language answer, stderr, and optional adapter-specific transport
metadata. The common adapter SHALL NOT require a ringi business-result schema.

#### Scenario: An agent answers through stdout
- **WHEN** ringi invokes an Agent CLI with a dossier question
- **THEN** the adapter returns the process outcome and stdout answer without interpreting its business meaning

### Requirement: Agent Invocation Uses Program And Arguments, Never A Shell
The subprocess adapter SHALL invoke the configured Agent CLI as a program plus an argument vector,
never through a shell. It SHALL set the configured working directory and SHALL clear the inherited
environment before adding the minimized base environment and explicit request environment.

#### Scenario: No shell interpretation
- **WHEN** an argument contains shell metacharacters
- **THEN** the configured program receives them as literal argument content

### Requirement: Agent Invocation Is Bounded By A Timeout
Every Agent CLI invocation SHALL have a wall-clock timeout. When the timeout expires, the adapter
SHALL terminate the invocation, report a timeout infrastructure outcome, and SHALL NOT fabricate an
answer.

#### Scenario: A hung agent is terminated
- **WHEN** an Agent CLI does not exit before its configured timeout
- **THEN** the adapter terminates it and reports a timed-out process outcome

### Requirement: Provider-Specific Structure Is Optional Transport Metadata
An adapter MAY parse Claude, Codex, or another CLI's JSON or event envelope to recover stdout text,
session identity, cost, or timing metadata. Missing or malformed optional structure SHALL NOT make a
usable natural-language stdout answer a business error. Provider metadata SHALL NOT become ringi's
dossier or arbitration schema.

#### Scenario: Plain text remains portable
- **WHEN** an Agent CLI exits successfully with a natural-language stdout answer and no structured envelope
- **THEN** the adapter returns the answer and leaves optional transport metadata absent

#### Scenario: A non-zero exit is not an answer
- **WHEN** an Agent CLI exits non-zero
- **THEN** the adapter reports the process outcome and captured streams without treating stdout as a completed deliberation turn
