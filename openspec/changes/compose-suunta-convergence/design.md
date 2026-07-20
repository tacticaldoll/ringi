## Context

Today the deliberation loop ([`deliberate_loop.rs`](../../../crates/ringi/src/deliberate_loop.rs)) reads `current_revision.readiness` to decide when to transition a dossier to `ReadyForDecision`. That boolean is authored by the arbitrator agent: `ArbitrationOutput.readiness` flows through `apply_arbitration` straight onto the successor revision, and `Revision.readiness` is persisted. This lets a model decide "we are done" — contradicting the standing decisions that the model has no decision authority and that mechanical facts outrank NLP claims.

suunta is a declared dependency (`suunta = "0.1.1"`) composed nowhere. Its shipped contract (`Bearing`/`Correction`/`Sigil`, `Fix`/`Sounding`/`SatisfactionFinding`, `plan_residual`, `Residual::is_converged`, and a conservative `Satisfaction::Unknown`) covers ringi's convergence need in full; ringi need only supply the per-target verdict. This change composes it, removes the agent's readiness authority, and gives risks the closed state that makes them honest suunta targets.

## Goals / Non-Goals

**Goals:**
- Readiness is `plan_residual(bearing, sounding).is_converged()`, never authored or stored by an agent.
- Both dissents and risks are honest targets: each has a provenance-bound resolution state, so `Satisfied` is a real, filterable verdict for both.
- Remove `readiness` from `ArbitrationOutput`, from `Revision`, and from the store.
- Keep `main` releasable: the live loop still runs a turn end to end via a labelled temporary stopgap.

**Non-Goals:**
- The `Motion` structured-move authorship mechanism (arbitrator still authors the whole successor `Revision` this slice).
- The prompt-width granularity knob tied to strategy.
- Open questions and conditions in the residual (v1 residual = dissents + risks).
- Any change to suunta itself.

## Decisions

### Decision: a thin `convergence` seam composes suunta; the Bearing is the full goal
A new `convergence` module maps a `&Revision` to `(Bearing<()>, Sounding)` and returns `is_ready(&Revision) -> bool`. It builds a `Correction::new(Sigil::new("dissent:<id>" | "risk:<id>"), Reversibility::Reversible, ())` for *every* dissent and risk (resolved or not) and a `Fix` with one `SatisfactionFinding` per target; `Sounding::new(fix, vec![])` (coverage unused in v1). Readiness is `plan_residual(bearing, sounding).is_converged()`.

- *Why a dedicated seam:* `Bearing`/`Sigil`/`Sounding` are suunta's register; docs/naming.md's seam rule keeps them out of ringi domain types. The seam is the one place they appear.
- *Why the full goal, not the residual:* if the `Bearing` held only unresolved targets, `plan_residual` would degenerate to an emptiness check — suunta as a glorified `is_empty()`, reimplementing convergence and leaving no way to express `Unknown`. `Body = ()` and `Reversibility::Reversible` are fixed because convergence reads neither; they must not carry meaning.

### Decision: every target is reported with an explicit finding
suunta omits a target from the residual only on *positive* certification; a target absent from the `Fix` is retained (surfaced as uncertain) and blocks `is_converged`. So the `Fix` carries one `SatisfactionFinding` per `Bearing` target — `Satisfied` for a provenance-bound resolution, `Unsatisfied` otherwise. Omitting satisfied targets would make a fully-resolved dossier read "not converged."

### Decision: derive verdicts from revision structure, not prose
A dissent or risk is `Satisfied` iff it has a provenance-bound `Resolution` (the `propose_successor` gate enforces reason + provenance for both). Ringi never infers satisfaction from agent text. In v1 the mapping emits only `Satisfied`/`Unsatisfied`; `Unknown` has no structural trigger yet and is a forward-compat guarantee tested by constructing a `Sounding` directly at the seam.

### Decision: risks mirror dissents (closed state + persistence)
`Risk { id: Uuid, description: String, resolved_by: Option<Resolution> }` replaces `unresolved_risks: Vec<String>`. `propose_successor` gains a risk gate identical to the dissent gate (no silent removal of an unresolved risk; resolution needs reason + provenance). Persistence adds a `risks` table and a `risk_resolution_provenance` table with commit-time event-reference verification and reload, mirroring `dissents`/`resolution_provenance`.

- *Why mirror rather than reuse:* dissents and risks are distinct domain concerns with identical mechanics; a shared table would blur them. The symmetry is cheap and keeps each concern addressable.
- *Alternative considered:* derive a risk `Sigil` by hashing its text. Rejected — text may be edited while denoting the same concern, breaking `Sigil` stability.

### Decision: remove `Revision.readiness` entirely, recompute on demand
Dropping only `ArbitrationOutput.readiness` would leave `Revision.readiness` authored (via `successor_revision`) and persisted — the authority vestige survives. So the field and its store column are removed; readiness is recomputed from the residual wherever needed (the loop, and `inspect`).

### Decision: evaluate readiness on the initial revision and each successor
The loop currently checks readiness only at the top of an iteration, so a successor produced on the final turn is never evaluated. The loop is restructured to (a) transition `Submitted -> Deliberating`, (b) check `is_ready` on the initial revision, and (c) after producing each successor, check `is_ready` and transition on convergence — so final-turn convergence transitions.

### Decision: a labelled temporary single-line-JSON stopgap keeps the loop runnable
`build_arbitrator_prompt` gains one clearly-labelled temporary line instructing the arbitrator to emit its successor-revision as **exactly one line of compact JSON, no surrounding prose**. This is required because the transport (`agent.rs::parse_metadata`) scans stdout lines in reverse for a single line that parses as JSON; multi-line pretty-printed JSON would not parse. Runnability therefore holds for a cooperative/fixture agent emitting compact single-line JSON; real-arbitrator robustness is explicitly the `Motion` slice's job, which deletes this stopgap.

## Risks / Trade-offs

- [`Sigil` instability if a risk id is not stable] → id assigned once at risk creation, carried forward by `propose_successor`, never re-derived from mutable text.
- [Forgetting a finding for a satisfied target silently breaks convergence] → the seam always emits one finding per target; a unit test asserts a fully-resolved dossier converges.
- [The single-line-JSON stopgap looks like endorsing whole-struct authorship] → labelled temporary in code and recorded in `BACKLOG.md` as owned by the `Motion` slice, which deletes it; runnability caveat stated above.
- [v1 residual omits questions/conditions, so a dossier could read "converged" while questions remain] → documented narrow scope; questions/conditions enter in the deferred residual-expansion slice. No silent widening.
- [Reopening: a resolved item that later reopens must leave `Satisfied`] → suunta is per-cycle; the next turn's `Sounding` reflects the reopened state, so `is_converged` flips back with no extra machinery.

## Migration Plan

- `ArbitrationOutput` and `Revision` drop `readiness`; the revisions table drops its `readiness` column. Risks change from `Vec<String>` to id-bearing `Risk`s with two new tables. No dossiers are in flight in the MVP, so the store is recreated rather than migrated.
- Existing fixtures/tests that build risks as strings or set `readiness` are updated.
- Rollback: revert the change set; suunta returns to an unused declared dependency.

## Open Questions

- None blocking. The `Sigil` scheme (`dissent:<uuid>`, `risk:<id>`) is internal and may be refined during apply without changing the spec.
