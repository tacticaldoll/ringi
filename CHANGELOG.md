# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres
to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-07-30

First release: the deliberation MVP end to end — draft, submit, deliberate, decide, archive —
composing pacta, suunta, and cadw. Structured move authorship (`Motion`) replaces whole-successor
arbitrator authorship; dissents, risks, questions, and conditions are all real `Revision` residual
items with create/resolve symmetry.

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
  declaration-level naming guard. Later extended to confine `pacta` to `registry` and `cadw` to
  `residual_ledger`.
- Revived `registry.rs`'s `SqliteRegistry`, a durable `pacta::Registry` claiming and settling each
  respondent, arbitrator, and condition-evaluator invocation, so a crash between invoking an Agent
  CLI and committing its result is durably distinguishable from a completed attempt.
- **Structured move authorship ("Motion")**: an arbitration turn no longer authors an entire
  successor `Revision`. It declares zero or more discrete, provenance-bound `Move`s —
  `AddDissent`, `ResolveDissent`, `AddRisk`, `CloseRisk`, `AskQuestion`, `AnswerQuestion` —
  applied atomically by `Revision::apply_moves`: one invalid move rejects the whole batch, matching
  the prior all-or-nothing turn behavior. `original_proposal`/`revision_id`/`parent_digest`/
  `content_digest` are no longer read from the agent at all, removing the immutability-check bug
  class structurally rather than validating it more strictly.
- `Question` joined `Dissent`/`Risk` as a real residual item with its own convergence target
  category and store persistence; the respondent prompt gained an `## Open Questions` section.
- `crate::residual_ledger`, a seam composing [`cadw`](https://crates.io/crates/cadw)'s `Ledger`
  for atomic batch-fold validation (existence, state-machine, duplicate-target rejection) of a
  `Move`/`ConditionMove` batch, replacing an inline implementation.
- `Condition` promoted from a dossier-level `{ id, description, is_met: bool }` flag to a
  `Revision`-level residual item, `{ id, description, resolved_by: Option<Resolution> }`,
  mirroring `Dissent`/`Risk`/`Question` — real reason/provenance storage an evaluator's verdict
  previously computed and then discarded. Mutated through a new `ConditionMove` (`Add`/`Satisfy`)
  — deliberately a separate enum from the arbitrator's `Move`, so an arbitrator can never author
  or satisfy a condition. Gains its own `conditions`/`condition_resolution_provenance` tables.
- The terminal archive renders dedicated `## Dissents`/`## Risks`/`## Questions`/`## Conditions`
  sections, each a checkbox list with an explicit placeholder when empty.
- The arbitrator prompt shows every unresolved item's stable id and each recent respondent
  claim's event id, so a later turn's `Move` can reference either as a target or as resolution
  provenance.

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
  uses a hardcoded literal. Later extended to cover `questions`, then `conditions`, as each joined
  the residual model.
- Adopted `cadw-contract` for `residual_ledger`'s validation, initially as a temporary Git
  dependency (cadw was unpublished), then flipped to the published `cadw` facade crate once cadw
  shipped its own `0.1.0` (contract and facade together) — matching how ringi already depends on
  `pacta`/`suunta`'s facades rather than their `-contract` crates directly.
- Upgrade pacta from 0.2.2 to 0.3.0 (`Registry`/`AsyncRegistry` relax their `Send + Sync`
  supertrait bound on the backend type; `pacta-executor` gains an opt-in `Policy`/`Verdict` trait
  for infrastructure-failure disposition that ringi's own `registry.rs` does not use).

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
- The terminal archive's `## Final SSOT` section only ever rendered `original_proposal`/
  `current_understanding` — every dissent, risk, and question (resolved or not) was silently
  omitted. Found by dogfooding a never-converging arbitrator whose accumulated, unresolved risks
  left no trace in the archive of a cancelled dossier; only `## Conditions` (added earlier) ever
  rendered an actual residual category.
- `Dissent` had no creation path anywhere in the live system: the arbitrator's `Move` enum had
  `ResolveDissent` but no `AddDissent`, unlike `Risk`/`Question`, which both pair an `Add*`/`Ask*`
  variant with their resolve/answer variant. Every dissent that existed anywhere in the codebase
  was inside a test's literal `Revision` construction, never reachable through any real dossier.

### Removed

- `shaahid`, a declared but entirely unused dependency: no code anywhere imported `shaahid::`
  anything. `InvocationCoordinate`'s own content-derived identity already closes the
  identity/content-drift gap shaahid exists to fill — a considered Decline, re-confirmed after
  Motion shipped and again after Conditions folded into the residual model, now with the leftover
  dependency itself actually removed rather than left as compiled dead weight.

[0.1.0]: https://github.com/tacticaldoll/ringi/releases/tag/v0.1.0
