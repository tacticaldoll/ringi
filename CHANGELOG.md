# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

_0.1.0 is in development; it has not been released._

### Added

- Project shape: the `ringi` app skeleton (a `clap` command surface, stubbed) and the
  self-driving foundation (`PROJECT.md`, `AGENTS.md`, `BACKLOG.md`, OpenSpec scaffolding).
  Behavior is built bet-first — first the minimal composition loop over the pacta family.
- `ringi evaluate <id>`: judges a `ReadyForDecision` dossier's unmet conditions with isolated
  `ConditionEvaluator` invocations, sealing the evaluator's reasoning from respondent/arbitrator
  context. `approve_with_conditions` can now actually reach plain `Approved` once every condition
  is met.
- A `tianheng`-backed architecture test (`crates/ringi/tests/architecture.rs`) mechanically
  confines `suunta`'s imports to the `convergence` seam, alongside `scripts/naming-guard.sh`'s
  declaration-level naming guard.
- Revived `registry.rs`'s `SqliteRegistry`, a durable `pacta::Registry` claiming and settling each
  respondent, arbitrator, and condition-evaluator invocation, so a crash between invoking an Agent
  CLI and committing its result is durably distinguishable from a completed attempt.

### Changed

- Replace the suunta and shaahid `release/0.1.0` Git dependencies with their published 0.1.1
  facade crates. Their public behavior is unchanged; the existing convergence, exactly-once,
  reclaim, restart, and agent-backed composition tests remain the compatibility gate.
- Upgrade pacta from 0.1.2 to 0.2.2 and migrate `SqliteRegistry` to the hardened backend-author
  surface: native atomic claim, lease accessor, and one transactional `apply` port over pacta's
  shared lifecycle decisions. The durable backend now passes sequential and contention conformance,
  including an independent-connection claim fence, with only an additive claim-selection index —
  no table or stored-row-format change — and no reconcile-loop behavior change.
- `Revision::compute_digest()` now hashes a revision's SSOT content (`original_proposal`,
  `current_understanding`, `positions`, `dissents`, `risks`) with SHA-256, instead of formatting
  the revision's own random `revision_id`; the initial revision `submit_command` creates no longer
  uses a hardcoded literal.

### Fixed

- Two unit tests that each mutate the process's current working directory could race under
  `cargo test`'s default parallel runner, since each was written and reviewed on its own branch
  where it was the only such test in its module; serialized with a crate-wide
  `PROCESS_CWD_LOCK` shared by every CWD-mutating or agent-spawning test.
- `claimed_invoke` settled a pact fulfilled on a zero exit code alone, before the caller had parsed
  the agent's output into the structured type it actually needed — a malformed arbitrator or
  condition-evaluator response (exit 0, unparseable output) was permanently marked settled,
  blocking every future retry. It now settles fulfilled only once the caller has a usable result.
- `SqliteRegistry::claim_invocation` scoped every invocation's pact to its dossier alone, so
  pacta's docket-scoped `claim()` could return a *different* coordinate's pact within the same
  dossier (e.g. a respondent's re-invocation silently claiming and fulfilling the arbitrator's
  still-pending one). Each pact's docket is now the coordinate's own idempotency key, so a docket
  never holds more than one pact.
- `Revision::propose_successor` now rejects a successor whose `original_proposal` differs from its
  parent's — previously nothing enforced this, so a buggy or malicious agent response could
  silently move the target a dossier is deliberating toward.
- `ringi inspect` reported `Readiness: true` for a freshly-submitted dossier before any turn had
  run, since it called `is_ready` alone instead of the same root-vs-successor rule
  `run_deliberation` already applies. Both now share `is_ready_for_decision`.
- `Cargo.toml`'s `description` (and so `ringi --help`'s top-level text) still described the
  pre-reframe "durable, gated build-review-verify loop" model; rewritten to describe the current
  dossier-deliberation model. A dossier draft file's frontmatter JSON is now always separated from
  both `---` delimiters by a newline, instead of glued as `}---`/`---{` on write and rewrite.
- A turn whose respondent succeeded but whose arbitrator then failed could never be resumed:
  retrying re-derived the respondent's already-`Settled` coordinate, which `claimed_invoke`
  correctly refused to reclaim, blocking the turn one step before the one that actually needed
  retrying. The respondent's answer is now persisted durably as soon as it succeeds, and a retry
  reuses it instead of re-invoking the respondent. Also: `apply_arbitration`'s domain validation
  (dissent/risk retention, `original_proposal` immutability) now runs inside the same claim
  boundary as the arbitrator's parse — a structurally-valid response that fails that validation
  releases the claim for retry instead of settling it fulfilled.
