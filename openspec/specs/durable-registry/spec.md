# durable-registry Specification

## Purpose

Ringi's SQLite-backed `pacta::Registry` — a durable claim/settle authority that satisfies
pacta's contract (conformance-proven) and over which the reconcile loop runs, so a run's step
lifecycle survives a restart. It is the sans-I/O seam made real: pacta owns the contract,
ringi owns the I/O.

## Requirements

### Requirement: SqliteRegistry Satisfies The Pacta Contract
Ringi SHALL provide `SqliteRegistry`, a durable `pacta::Registry` implementation backed by
SQLite, that passes the `pacta-conformance` suite in full — so it satisfies the lifecycle
contract by the same standard as the reference backend. It SHALL implement claim, heartbeat,
fulfill, breach, and release with pacta's lease, lapse, authority-rotation, and
deferred-reclaim semantics, and SHALL reimplement no lifecycle policy of its own.

#### Scenario: The SQLite backend passes conformance
- **WHEN** `pacta-conformance` runs against `SqliteRegistry`
- **THEN** every scenario passes, including lapse recovery, authority rotation, and deferred reclaim

#### Scenario: Only the current holder settles
- **WHEN** a retainer that is not the current holder attempts to settle, heartbeat, or release
- **THEN** the registry rejects it, preserving at-least-once safety across a reclaim

### Requirement: The Registry Injects Time And Reads No Clock
`SqliteRegistry` SHALL take the current time as a parameter on the calls that need it and
SHALL read no ambient clock, storing and comparing injected instants only — mirroring
pacta's sans-I/O contract.

#### Scenario: Lease decisions use the injected instant
- **WHEN** claim or heartbeat is called with a `now` value
- **THEN** eligibility and expiry are computed from that value, not from a wall clock the registry reads

### Requirement: The Reconcile Loop Runs Over A Durable Backend
The reconcile loop SHALL be able to run over `SqliteRegistry` unchanged at its seam, so the
composition proven over the reference backend now persists a run's step lifecycle. Swapping
the backend SHALL require no change to the loop's planning, witnessing, or settlement wiring.

#### Scenario: The composition is backend-agnostic
- **WHEN** the reconcile loop runs over `SqliteRegistry` instead of the reference backend
- **THEN** it converges with the same exactly-once and retry behavior, now durably

#### Scenario: State survives a restart
- **WHEN** a file-backed registry holds a claim, is dropped, and is reopened from the same file
- **THEN** the held state persisted, so the pact is reclaimable after its lease lapses
