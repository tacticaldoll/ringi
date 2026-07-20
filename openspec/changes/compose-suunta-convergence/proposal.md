## Why

Readiness — whether a dossier has converged and may go to a human decision — is currently a boolean the arbitrator agent writes into its own output and that the loop obeys verbatim. That violates two standing decisions: the model exercises no decision authority, and mechanical facts outrank NLP claims. It also leaves suunta, the sibling that exists to own convergence over a residual, declared as a dependency but composed nowhere. This change makes convergence a mechanical fact computed by suunta and removes the agent's authority over "we are done."

## What Changes

- Compose suunta 0.1.1 to compute readiness. Each turn, ringi builds a `Bearing` whose `Correction`s are every deliberation target (every dissent and every risk, keyed by a stable `Sigil`), a `Sounding` whose `Fix` carries one `Satisfaction` verdict per target, and derives readiness as `plan_residual(bearing, sounding).is_converged()`. This lives in a thin `convergence` seam so suunta vocabulary never leaks into ringi's domain types.
- Every target gets exactly one finding, and `Unknown` is conservatively retained, so a fully-satisfied residual converges and an uncertain one never does.
- **BREAKING** Remove `readiness` from `ArbitrationOutput` **and** from `Revision`, and drop the `readiness` column from the revisions table. Readiness ceases to be an agent output and is no longer a stored field; it is recomputed from the residual. The `ReadyForDecision` transition is driven only by suunta.
- Give risks a real closed state: a `Risk` gains a stable id and an optional provenance-bound `Resolution` (mirroring `Dissent`), so a resolved risk is `Satisfied` and an open one is `Unsatisfied`. `propose_successor` enforces conservative retention for risks as it already does for dissents.
- Build risk persistence from scratch: a `risks` table and a `risk_resolution_provenance` table, with commit-time event-reference verification and reload — mirroring the existing dissent persistence. (Risks are not persisted at all today; `get_latest_revision` returns none.)
- Fix the loop so a revision that converges on the final turn still transitions: readiness is evaluated on the initial revision and on every freshly-produced successor, not only at the top of the next iteration.
- Add a temporary, explicitly-labelled one-line instruction to the arbitrator prompt asking it to emit its successor-revision as a single line of compact JSON, so the line-scanning transport can parse it and the live loop runs again. This stopgap is owned by, and deleted with, the future `Motion` slice.

Out of scope (deferred in `BACKLOG.md`): the `Motion` structured-move authorship mechanism; the prompt-width granularity knob tied to strategy; open questions and conditions entering the residual; any change to suunta itself.

## Capabilities

### New Capabilities
<!-- none: convergence lives in the existing deliberation-loop capability -->

### Modified Capabilities
- `deliberation-loop`: readiness is computed mechanically by suunta over the residual each turn, not asserted by the arbitrator; every target carries a finding; `Unknown` never converges; final-turn convergence transitions; the arbitrator no longer outputs readiness and `Revision` no longer stores it.
- `deliberation-dossier`: each risk carries a stable id and an optional provenance-bound resolution, is conservatively retained, and is persisted and reloaded; the residual comprises unresolved dissents and unresolved risks.

## Impact

- Code: `crates/ringi/src/deliberation.rs` (`ArbitrationOutput`, `apply_arbitration`, `build_arbitrator_prompt`, `build_respondent_prompt`), new `crates/ringi/src/convergence.rs` (suunta seam), `crates/ringi/src/deliberate_loop.rs` (readiness placement, transition), `crates/ringi/src/revision.rs` (`Risk` type, `readiness` removal, risk retention gate), `crates/ringi/src/store.rs` (risks tables, drop readiness column), `crates/ringi/src/dossier_cli.rs` (initial revision, inspect output).
- Dependencies: composes `suunta = "0.1.1"` (already declared); no suunta change.
- Migration: `ArbitrationOutput` and `Revision` drop `readiness`; the revisions table drops its `readiness` column; risks change from `Vec<String>` to id-bearing `Risk`s with new tables. No dossiers are in flight in the MVP, so no data migration is required; the store is recreated.
- Verification: unit tests for the `convergence` seam (a `Revision` maps to targets/findings and `is_ready` is correct, including a satisfied-target-omission case and a seam-level `Unknown` case), risk persistence round-trip tests, and an end-to-end fixture run of the loop reaching `ReadyForDecision` via suunta.
