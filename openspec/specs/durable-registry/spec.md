# durable-registry Specification

## Purpose

Ringi's SQLite-backed `pacta::Registry` — a durable claim/settle authority that satisfies
pacta's contract (conformance-proven) and over which the reconcile loop runs, so a run's step
lifecycle survives a restart. It is the sans-I/O seam made real: pacta owns the contract,
ringi owns the I/O.

## Requirements

### Requirement: SqliteRegistry Satisfies The Pacta Contract
Ringi SHALL provide `SqliteRegistry`, a durable `pacta::Registry` implementation backed by
SQLite, that passes pacta 0.2.2's sequential and synchronous contention conformance runners — so
it satisfies the lifecycle contract by the same standard as the reference backend. It SHALL
implement the native atomic `claim` selection, the `lease_millis` accessor, and one atomic `apply`
transition port; heartbeat, fulfill, breach, and release SHALL be inherited from pacta's defaults.
`apply` SHALL locate the row held by the presented retainer, run the passed shared
`pacta::lifecycle` transition, and persist its returned state in one SQLite transaction, deciding no
lifecycle outcome of its own. Claim SHALL re-express `lifecycle::is_claimable` as a native,
parameterized SQLite query backed by a claim-selection index, so it is full-scan-free, and derive
its held state from pacta's shared lifecycle semantics.

#### Scenario: The SQLite backend passes sequential conformance
- **WHEN** `pacta_conformance::run` runs against `SqliteRegistry`
- **THEN** every lifecycle scenario passes, including exact heartbeat expiry, lapse recovery, authority rotation, and deferred reclaim

#### Scenario: The SQLite backend passes contention conformance
- **WHEN** `pacta_conformance::run_contention` runs against `SqliteRegistry`
- **THEN** concurrent claim and settlement contention each produce exactly one winner through the public Registry contract

#### Scenario: Shared lifecycle decisions cross the atomic apply port
- **WHEN** pacta's default heartbeat, fulfill, breach, or release operation passes a transition to `SqliteRegistry::apply`
- **THEN** the registry loads the held state, executes that transition, and persists its returned state in one SQLite transaction without reimplementing the operation's decision

#### Scenario: Claim selection stays native and boundary-faithful
- **WHEN** claim selects among available, held, deferred, and settled rows at an injected `now`
- **THEN** its indexed, parameterized SQLite predicate admits exactly available rows, held rows with expiry strictly before `now`, and deferred rows at or before their reclaimable instant, while excluding settled rows without scanning the full table

#### Scenario: Only the current holder can transition
- **WHEN** an unknown, released, settled, or reclaim-fenced retainer reaches `apply`
- **THEN** the registry rejects it as not held and changes no row

#### Scenario: Independent connections cannot double-claim
- **WHEN** two independently opened `SqliteRegistry` connections concurrently claim the single eligible pact in one database
- **THEN** the SQLite transaction boundary permits exactly one claim and the other attempt returns no claim

### Requirement: The Registry Injects Time And Reads No Clock
`SqliteRegistry` SHALL take the current time as a parameter on the calls that need it and
SHALL read no ambient clock, storing and comparing injected instants only. Claim SHALL use the
injected instant in its native eligibility query and pacta lifecycle arithmetic; heartbeat SHALL
receive its instant through pacta's default operation and shared transition.

#### Scenario: Lease decisions use the injected instant
- **WHEN** claim or heartbeat is called with a `now` value
- **THEN** eligibility and expiry are computed from that value and pacta's shared lifecycle semantics, not from a wall clock the registry reads

#### Scenario: An instant outside SQLite's exact range is rejected
- **WHEN** claim or a transition would persist a pacta timestamp greater than SQLite's signed 64-bit integer range
- **THEN** the registry returns a distinct range error and changes no lifecycle row rather than saturating the instant and changing its boundary

### Requirement: The Reconcile Loop Runs Over A Durable Backend
The reconcile loop SHALL run over the pacta 0.2.2 `SqliteRegistry` without changing its planning,
witnessing, retry, convergence, or settlement wiring, so the composition proven over the reference
backend persists a run's step lifecycle. The dependency migration SHALL preserve the existing table
and stored-row representation; it MAY add an idempotent claim-selection index, which SHALL be
provisioned when an existing database opens without rewriting lifecycle data.

#### Scenario: The composition is backend-agnostic
- **WHEN** the reconcile loop runs over the upgraded `SqliteRegistry` instead of the reference backend
- **THEN** it converges with the same exactly-once and retry behavior, now durably

#### Scenario: State survives a restart
- **WHEN** a file-backed registry holds a claim, is dropped, and is reopened from the same file after the dependency upgrade
- **THEN** the held state remains readable and is reclaimable after its lease lapses after the additive index is provisioned, without a table, row-format, or data migration
