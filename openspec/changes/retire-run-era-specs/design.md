# Design

## Context

See proposal.md for motivation. The surviving concepts of the retired capabilities are already
specified elsewhere: suunta convergence and no model readiness authority by `deliberation-loop`;
claim/settle, release for retry, and turn resumption by `resumable-dossiers` and
`durable-registry`; no shell and timeouts by `agent-adapter`; the human decision by
`human-decision`. Each REMOVED entry's Migration names its home, or says None where the behavior
has no counterpart.

## Goals / Non-Goals

**Goals:**
- `openspec/specs/` describes only code that exists.
- Nothing the code still does loses its specification.
- `openspec validate --specs --strict` passes.

**Non-Goals:**
- Changing product code or code comments.
- Reconstructing the placeholder specs' intended requirements.

## Decisions

- **REMOVED deltas, then delete the emptied specs at sync.** A delta can remove every requirement
  but cannot delete a spec file, so sync removes each spec directory whose requirements are all
  removed.
- **Delete the placeholders at sync.** They have no requirements, so no REMOVED delta can target
  them. Sync deletes their directories directly.
- **Restate, do not move, the two `durable-runs` requirements.** Their run wording (`ringi run`,
  recorded runs, "user-scope") is wrong for the code, so they are ADDED to `dossier-cli` in dossier
  terms. The init requirement claims the `.ringi/` directory, `state.sqlite`, and the dossier
  schema, created with `CREATE TABLE IF NOT EXISTS`. The shared-store scenario names the commands
  that open the registry (`ringi continue`, `ringi evaluate`).
- **Leave `AgentRole::Builder`/`Reviewer` alone.** Removing unused code is a code change, not a
  spec retirement.

## Risks / Trade-offs

- [A retired requirement described behavior that still exists somewhere] → Each retired spec's
  requirements are checked against `crates/ringi/src`; the only remnants are the unused
  `AgentRole` variants, which specify nothing.
