# Proposal

## Why

Ringi assessed shaahid and declined it (`BACKLOG.md` "Family Dependency Stance", `PROJECT.md`),
then removed the dependency: no source file imports it and neither manifest declares it. The
shipped specs still describe step attempts witnessed with shaahid, a shaahid `Seal`/`Fingerprint`
attempt identity, and a shaahid witness ledger rebuilt on resume, so `openspec/specs/` claims a
composition the code does not have.

## What Changes

- `reconcile-loop`: remove "Each Step Executes Exactly Once" and "The Exactly-Once Attempt
  Identity Is Distinct From The Target Identity". Drop the shaahid clauses, and the exactly-once
  assertions that rested on shaahid, from the composition, self-check, and agent-backed round-loop
  requirements and from the Purpose.
- `resumable-runs`: drop the witness-ledger reconstruction and its "Exactly-once holds across a
  restart" scenario, and the shaahid sentences in the Purpose. OpenSpec cannot drop one scenario
  through MODIFIED, so the requirement is replaced whole, as "An Interrupted Run Resumes From Its
  Recorded Checkpoint", with the rest of its text unchanged.
- `naming-worldview`: the brick-vocabulary examples name cadw (`TargetId`/`Ledger`/`Validator`),
  which ringi actually imports, instead of shaahid.
- Delete `sync_specs.py`, a one-off sync helper hard-wired to
  `openspec/changes/reframe-ringi-deliberation`, a directory that no longer exists. `AGENTS.md`
  makes sync agent-driven, so nothing calls the script and it would misfire if run.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `reconcile-loop`: the loop no longer claims shaahid composition or an exactly-once witness.
- `resumable-runs`: resume no longer claims a reconstructed witness ledger.
- `naming-worldview`: the brick-vocabulary examples name a brick ringi composes.

## Impact

Specs only, plus the removal of the unused root script `sync_specs.py`. No product code,
manifest, or lockfile changes. Invocation idempotency is already specified by
`durable-registry` and `resumable-dossiers` (the content-derived invocation coordinate claimed
through the durable pacta registry), so nothing the code does loses its specification.
