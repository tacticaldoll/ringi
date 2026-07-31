# Backlog & Deferred Decisions

Records the plan, deferred decisions, and design knowledge so the repo can drive its own
development. Shipped truth lives in `openspec/specs/`; active proposed truth in
`openspec/changes/`.

## Current Baseline

Project shape established: the app skeleton (`crates/ringi`, a `clap` CLI whose commands are
stubbed), `PROJECT.md`, `AGENTS.md`, and OpenSpec scaffolding. No behavior yet — built
bet-first (below).

## The Bet (gates everything)

The family thesis is that an orchestrator *emerges from composing* thin bricks. Ringi is the
first real test. Before scaling into the product, prove — cheaply — that the composition
carries its weight with acceptable friction:

- A **minimal reconcile loop** wiring suunta (plan the residual over open work) -> shaahid
  (witness each step so it runs exactly once) -> pacta (durable claim -> execute -> settle,
  with `release(reclaimable_at)` for backoff'd retry) -> back to suunta, driven by the
  consumer, over stub agents and the `pacta-memory` reference backend.
- Success = the consumer only *wires* (identity mapping, observe->findings translation, the
  loop); the bricks do the hard parts. If ringi ends up reimplementing coordination, the
  bet is failing and the design must be revisited before investing further.

Only after the bet holds do the phases below get built.

**Bet outcome — held (recorded).** `reconcile::run` (+ its self-checking test) composes
suunta + shaahid + pacta through their public APIs into a convergent, exactly-once, durable
reconcile. The consumer stayed thin: the loop plus thin seam adapters (Sigil↔Uuid bridge,
step↔Pact, step↔Deed, satisfaction findings) — no `Run`/`Step` engine, completion
calculation, or idempotency scheme of ringi's own. Exactly-once was proven across **both**
failure paths that matter: a step that fails once retries via pacta `release(reclaimable_at)`
(backoff is the consumer's), and a step whose settlement is lost lapses, is reclaimed, and is
**not** re-executed because shaahid `witness` returns `Attach`. Convergence is decided by
suunta, never a ringi check. The interlocks proven at the type level held at runtime
(Sigil==Seal; per-step docket for individual claim; outcome→satisfaction). The thesis —
orchestrator emerges from composing thin bricks — is validated on a real consumer. Scaling
into the phases is now warranted.

## Family dependency stance

- **pacta** is published (`0.1.2`, crates.io) — depend on it normally; it provides the
  `Registry` contract, `release`, and `pacta-conformance`.
- **suunta** and **shaahid** are unpublished (each at `release/0.1.0`, held pending this
  bet). Depend on them as **git dependencies** (`branch = "release/0.1.0"`) until they
  release, then switch to crates.io. This repo is what validates the bet that lets them ship.

## Seam design (from the boundary work)

Two cut-lines that must hold (see `PROJECT.md`):

- **sans-I/O Registry seam.** pacta provides only the `Registry` trait (synchronous, zero
  I/O) + `pacta-conformance`. Ringi implements its own `SqliteRegistry: pacta::Registry`
  (sync, rusqlite) and proves it with the conformance suite. **One** user-scope SQLite DB
  holds both the Registry's lease/lifecycle state and ringi's domain tables
  (runs/steps/reviews/events/artifacts/approvals). Registry ops are sync and short (no
  transaction spans a subprocess); the slow async work (Agent CLI subprocess) is tokio, in
  ringi's own executor layer, separate from the Registry. Do **not** use pacta's sync
  reference `Driver` for the async steps — drive the `Registry` contract from a
  consumer-owned async loop (pacta permits this).
- **core-mechanism / edge-policy.** Policy/approval starts as ringi consumer code
  (allow/ask/deny + an approval gate). Only the gating *mechanism* might later extract into a
  Freigabe brick; the policy *content* (which actions deny) is forever ringi's. Do not
  pre-build Freigabe — force-then-extract.

Interlocks proven at the type level: suunta `Sigil` == shaahid `Seal` (one domain identity);
pacta outcome -> suunta next-cycle finding (`Fulfilled`->satisfied, `Breached`->unsatisfied,
infra-failure->unknown); shaahid `Attestation` gates whether ringi executes a step. Known
friction: pacta `Pact.id` is a `Uuid` while the domain identity is a string `Sigil`/`Seal` —
bridge with a deterministic v5 UUID or a map, carrying the `Sigil` in the pact's clause.
`pacta-memory` has no runtime enqueue (pacts enter via `seeded` at construction), so the
minimal loop seeds its steps; ringi's real `SqliteRegistry` provides its own idempotent
creation (ingress is backend/consumer territory, by pacta's design).

## Phased plan (after the bet)

1. **Local flow**: CLI, subprocess runner, Builder/Reviewer adapters, `git diff`,
   verification runner, in-memory reconcile loop.
   - **Agent seam — landed.** `agent::AgentAdapter` + `SubprocessAdapter`: program+args
     (never a shell), workspace cwd, minimized env, timeout-bounded (kill + `TimedOut`),
     concurrent stdin/stdout/stderr, best-effort trailing-JSON parse; non-zero exit reported,
     not raised. Fake-agent test covers success/non-zero/malformed/timeout.
   - Still pending on this surface: Builder/Reviewer role wiring into the loop, `git diff`
     capture, the verification runner, and the CLI beyond stubs.
2. **Persistence**: `SqliteRegistry` over pacta's contract (conformance-proven) + ringi's
   domain tables; runs/steps/events/artifacts; resume.
   - **Registry half — landed.** `store::SqliteRegistry` implements `pacta::Registry` over
     rusqlite (sync, injected time) and **passes `pacta-conformance`** — the first external
     backend to do so, validating pacta's "durable backends live outside and prove
     themselves" claim. The reconcile loop is parameterized over the backend (`run_with`) and
     runs the identical composition durably; a reopen test proves state survives a restart.
     Still pending on this surface: ringi's **domain tables** (runs/steps/reviews/events/
     artifacts/approvals) in the same DB, and a file-backed **resume** of a full run.
3. **Policy & approval**: Action normalization, allow/ask/deny, workspace path guard, the
   approval CLI, action-hash binding, audit log.
4. **Isolation & security**: git worktree, clean environment, secret redaction, output
   limits, network restriction. Includes **full process-tree teardown**: the timeout path
   currently kills the direct child, which suffices for an agent invoked directly but leaks a
   shell-wrapper agent's un-exec'd grandchildren (they can hold a pipe open); process-group
   spawn + group-kill closes that gap and belongs with the rest of process isolation.
5. **Productization**: multiple Agent CLI adapters, config validation, richer inspect,
   optional daemon / local HTTP.

## Ringi forces family growth

Ringi is the client that forces (and will help extract) the family's next surfaces. The
[round-model target](docs/round-model.md) audits how, and refines the picture: the disposition
of what a brick surfaces is an **embeddable execution Layer** (the Tower shape), and pacta's
`Middleware<E>` is the seed slot. That collapses what used to be planned as three separate
bricks into **one Layer discipline (mechanism) plus a few edge-policies**:

- the **retry cluster** (a retry Layer + user `Policy` trait) over pacta's seam — pacta left it
  deliberately unbuilt pending a client; this is the **first Shape-A Layer** ringi forces out;
- **Freigabe** (gate before execute), **Dychwel** (compensate on failure), **Stoma** (trip on
  repeated failure) — **not three bricks, but three edge-policies on the one Layer discipline**;
  each "wraps the execute." Force them out with real instances; do not pre-build.

Red lines (from the target): no general Layer trait / central policy engine everything routes
through (that is the central framework the vision forbids); and the fold over a brick's
`surfaced`/`contradictions` (suunta coverage, shaahid drift) is **policy content — forever
consumer `match`, never abstracted into a product**.

Audit result on identity: the exactly-once unit is an **attempt** (`<run>:<target>:<round>:
<attempt>`), which fits shaahid unchanged — **no family feedback needed** (the earlier "drift"
worry was an artifact of choosing the wrong unit, and dissolved at the attempt grain).

## Open questions (with recommended defaults)

First Agent CLI: support one first. Output: JSON. Reviewer: read-only. Workspace: git
worktree. Repository: require clean. Package install: requires approval. Approval: via CLI.
Arbitrary network: denied. Platform: Linux/macOS first. Monorepo: not special-cased.
Verification commands: config-supplied. Full model transcripts: not stored (only necessary
outputs). Completion: emit a patch. GitHub integration: later. Concurrency: one run at a
time in v1.
