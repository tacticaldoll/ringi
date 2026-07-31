# naming-worldview Specification

## Purpose

Ringi's authored naming register, the rule confining brick vocabulary to the seam, and the
executable guard against queue-runtime / CQRS drift — because semantic drift precedes
architectural drift, and a queue-runtime name is the first visible sign of re-monolithing.
See `docs/naming.md`.

## Requirements

### Requirement: Ringi Has A Native Naming Register
Ringi SHALL name its own domain in a clear deliberative-governance register (the arc
propose → review → verify → sanction → approve), documented in `docs/naming.md`. Brick
vocabulary (pacta `Pact`/`Registry`/`release`, suunta `Bearing`/`Course`, shaahid
`Deed`/`witness`) SHALL appear only in the thin seam adapters that call those crates, and
SHALL NOT name ringi's own domain types or modules. Ringi is an application, so clarity
outranks evocativeness.

#### Scenario: Ringi domain names use ringi's register
- **WHEN** a ringi domain type, module, or role is named
- **THEN** it uses ringi's deliberative register, not a brick's term nor a generic queue-runtime term

#### Scenario: Brick terms stay at the seam
- **WHEN** a brick's vocabulary appears in ringi
- **THEN** it appears only in the seam adapters that call that brick, not in ringi's own domain surface

### Requirement: Queue-Runtime Vocabulary Is Guarded
A queue-runtime or CQRS word SHALL NOT name a ringi domain type, module, or trait — this
covers workflow, job, queue, worker, broker, dispatcher, pipeline, scheduler, runner,
tenant, and the like. An executable guard SHALL enforce this and run in the Definition of
Done, so the drift that precedes re-monolithing fails the gate mechanically.

#### Scenario: A banned domain name fails the gate
- **WHEN** a queue-runtime word is introduced as a ringi type, module, or trait name
- **THEN** the naming guard fails under the Definition of Done

#### Scenario: The guard does not flag legitimate vocabulary
- **WHEN** the guard runs against the current source
- **THEN** it passes, and it does not flag standard CLI vocabulary such as `Command`
