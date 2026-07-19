# durable-runs Specification

## Purpose

How a run's state is made durable: ringi persists a run's identity, outcome, and open findings to
one user-scope SQLite store — the same database that holds the Registry's lease/lifecycle state —
so a run survives the process that produced it and can be read back later. `init` provisions the
store; `status` reads a persisted run. Recording is done in ringi's driver layer, never in the
composition root (`run-assembly` stays non-persisting). This is the "store is the source of truth"
invariant's first footing; resume (re-entering an interrupted run) is a later increment.

## Requirements

### Requirement: A Run's State Is Persisted To The One Durable Store
Ringi SHALL persist a run's state — its identity, task, workspace, terminal outcome (converged or
did-not-converge), round count, and open findings — to a durable, file-backed SQLite store, so the
record survives the process that produced it. The store SHALL be **one** user-scope SQLite DB that
also holds the Registry's lease/lifecycle state; ringi's domain tables and the Registry's tables
live in the same database. The run SHALL be recorded before its round loop begins and updated with
its outcome after, so a run interrupted before completion is observably not-complete rather than
absent. Persistence SHALL be performed by ringi's own driver layer, not by the composition root
(`run_from_config` continues to neither persist nor present — see the `run-assembly` capability).

#### Scenario: A completed run's record survives the process
- **WHEN** a run is driven to a terminal outcome and the process then exits
- **THEN** a later process reading the store finds the run with its outcome, round count, and open findings

#### Scenario: An interrupted run is observably not-complete
- **WHEN** a run is recorded and then the process ends before the run reaches a terminal outcome
- **THEN** the store shows the run as still running (not converged and not failed), never as absent

#### Scenario: Registry and domain state share one database
- **WHEN** a run persists its domain state
- **THEN** it is written to the same SQLite database that holds the Registry's lease/lifecycle state, not a second store

### Requirement: Init Provisions The Durable Store
The `ringi init` command SHALL provision the durable store — creating the SQLite database and its
schema — so that a subsequent `ringi run` has a store to record into. Provisioning SHALL be
idempotent: running `init` when the store already exists SHALL leave existing data intact.

#### Scenario: Init creates the store
- **WHEN** `ringi init` runs where no store exists
- **THEN** it creates the SQLite database and schema for the durable store

#### Scenario: Init does not destroy existing run data
- **WHEN** `ringi init` runs where the store already exists
- **THEN** it leaves the existing store and its recorded runs intact

### Requirement: Status Reads A Persisted Run
The `ringi status <run_id>` command SHALL read a run from the durable store and present its state
(running, converged, or did-not-converge), its round count, and its open findings. A run id that
is not present in the store SHALL produce a clear diagnostic and a non-zero exit status, never a
fabricated or empty-but-successful result.

#### Scenario: Status presents a recorded run
- **WHEN** `ringi status` is given the id of a run present in the store
- **THEN** it prints that run's state, round count, and open findings

#### Scenario: Status of an unknown run fails clearly
- **WHEN** `ringi status` is given an id not present in the store
- **THEN** it prints a clear diagnostic and exits non-zero
