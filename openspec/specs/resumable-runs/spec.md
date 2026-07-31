# resumable-runs Specification

## Purpose

How an interrupted run is continued: the round loop checkpoints its progress durably as it runs —
which build attempts succeeded and, per round, the next round to run with the findings still open —
and a resume re-enters from that checkpoint. Completed rounds are not re-executed, and the witness
ledger is reconstructed from the recorded rounds so exactly-once holds **across** a restart, not
only within a process. The ledger is ringi's own durable state; shaahid's `witness` stays sans-I/O
(no family change). Checkpointing is a no-op-default seam on the loop, so the in-memory composition
is unchanged.

## Requirements

### Requirement: A Run Checkpoints Its Progress Durably
As a run's round loop executes, ringi SHALL record to durable storage, per round, which build
attempts have succeeded and a checkpoint identifying the next round to run and the findings still
open. A succeeded build attempt SHALL be recorded **before** its durable lease is settled, so a
crash between a build's success and its settlement does not lose the record. Recording SHALL be
idempotent, so re-recording the same round during a resumed pass is harmless. Checkpointing SHALL
be an internal seam of the round loop with a no-op default, so a run driven without durable storage
behaves exactly as before.

#### Scenario: A succeeded build is recorded before settlement
- **WHEN** a round's build attempt succeeds
- **THEN** the attempt is recorded durably before the round's lease is settled

#### Scenario: The in-memory composition is unchanged
- **WHEN** the round loop is driven without a durable journal
- **THEN** it behaves identically to before checkpointing existed

### Requirement: An Interrupted Run Is Resumed From Its Checkpoint
Ringi SHALL be able to re-enter an interrupted run from its durable checkpoint: it SHALL resume at
the recorded next round with the recorded open findings, and SHALL NOT re-execute the builds of
rounds already completed nor re-run their reviews. The witness ledger SHALL be reconstructed from
the recorded succeeded rounds, so a reclaimed in-flight attempt is recognized as already performed
and attaches rather than re-executing. Resuming SHALL drive the run to convergence or the round
limit and record the terminal outcome.

#### Scenario: Completed rounds are not re-executed on resume
- **WHEN** a run interrupted after completing some rounds is resumed
- **THEN** the builds of those completed rounds do not execute again and their reviews are not re-run

#### Scenario: Exactly-once holds across a restart
- **WHEN** a build attempt succeeded but its settlement was lost to the interruption, and the run is resumed
- **THEN** the reclaimed attempt attaches to the recorded deed and its build side effect does not execute a second time

#### Scenario: A resumed run reaches a terminal outcome
- **WHEN** an interrupted run is resumed
- **THEN** it continues from the checkpoint to convergence or the round limit and its outcome is recorded

### Requirement: The Resume Command Continues An Interrupted Run
The `ringi resume <run_id>` command SHALL load an interrupted run from the durable store and
continue it. A run id that is not present, or a run that is not in a resumable (still-running)
state, SHALL produce a clear diagnostic and a non-zero exit status. On completion the command SHALL
present the run's outcome and map convergence to the exit status, as `ringi run` does.

#### Scenario: Resume continues a running run
- **WHEN** `ringi resume` is given the id of a run recorded as still running
- **THEN** it continues the run from its checkpoint and presents the resulting outcome

#### Scenario: Resume of an unknown or already-finished run fails clearly
- **WHEN** `ringi resume` is given an unknown id or the id of a run already at a terminal outcome
- **THEN** it prints a clear diagnostic and exits non-zero
