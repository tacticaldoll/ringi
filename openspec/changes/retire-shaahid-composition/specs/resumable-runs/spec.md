## REMOVED Requirements

### Requirement: An Interrupted Run Is Resumed From Its Checkpoint
**Reason**: its witness-ledger reconstruction and "Exactly-once holds across a restart" scenario
rest on shaahid, which ringi declined and removed. OpenSpec cannot drop one scenario through
MODIFIED, so the requirement is replaced whole.
**Migration**: Use "An Interrupted Run Resumes From Its Recorded Checkpoint", which keeps the rest
of this requirement's text unchanged.

## ADDED Requirements

### Requirement: An Interrupted Run Resumes From Its Recorded Checkpoint
Ringi SHALL be able to re-enter an interrupted run from its durable checkpoint: it SHALL resume at
the recorded next round with the recorded open findings, and SHALL NOT re-execute the builds of
rounds already completed nor re-run their reviews. Resuming SHALL drive the run to convergence or
the round limit and record the terminal outcome.

#### Scenario: Completed rounds are not re-executed on resume
- **WHEN** a run interrupted after completing some rounds is resumed
- **THEN** the builds of those completed rounds do not execute again and their reviews are not re-run

#### Scenario: A resumed run reaches a terminal outcome
- **WHEN** an interrupted run is resumed
- **THEN** it continues from the checkpoint to convergence or the round limit and its outcome is recorded
