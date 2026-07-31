# resumable-dossiers Specification

## Purpose

How an interrupted dossier deliberation is resumed correctly: the latest committed revision is the
resume point (see `deliberation-dossier`), and each Agent-CLI invocation within a turn is claimed
through the durable registry (`durable-registry`) before it runs and settled after, so a crash
between invoking the agent and committing its result is durably distinguishable from a completed
attempt rather than silently re-invoked or silently lost.

## Requirements

### Requirement: An Agent-CLI invocation is claimed before it runs and settled after

Before invoking an Agent CLI for a respondent, arbitrator, or condition-evaluator turn, ringi
SHALL claim a durable pact keyed by that invocation's `InvocationCoordinate` through
`SqliteRegistry`, and SHALL invoke the agent only if the claim succeeds. After the agent responds,
ringi SHALL settle the claim — fulfilled on success, released for the same coordinate on failure —
so a crash between the invocation and the eventual event/revision commit leaves a durable,
inspectable trace instead of none.

#### Scenario: A successful invocation is claimed then fulfilled

- **WHEN** an Agent-CLI invocation for a given coordinate is claimed and the agent responds successfully
- **THEN** the claim is settled fulfilled before the turn proceeds

#### Scenario: A failing invocation is released and stays retryable under the same coordinate

- **WHEN** an Agent-CLI invocation for a given coordinate is claimed and the agent invocation fails
- **THEN** the claim is released, so the same coordinate remains claimable — retrying is simply
  re-running the command, as it is today

#### Scenario: Re-submitting the same coordinate is idempotent

- **WHEN** a pact is submitted for a coordinate that was already submitted
- **THEN** no duplicate pact is created — the same underlying pact is claimed

### Requirement: An unclaimable invocation surfaces as an error, not a silent retry

If claiming a pact for an invocation's coordinate does not succeed — because it is already
settled, held under an unexpired lease, or deferred and not yet reclaimable — ringi SHALL surface
this as an error naming the coordinate rather than silently invoking the agent anyway or guessing
an outcome.

#### Scenario: An unclaimable coordinate does not invoke the agent

- **WHEN** claiming an invocation's coordinate does not produce a usable claim
- **THEN** the agent is not invoked for that coordinate and the caller receives an error identifying the coordinate
