## 1. Risk gains a closed state (mirror dissent)

- [ ] 1.1 In `revision.rs`, add `Risk { id: Uuid, description: String, resolved_by: Option<Resolution> }` and replace `Revision.unresolved_risks: Vec<String>` with `risks: Vec<Risk>`.
- [ ] 1.2 Extend `propose_successor` with a risk gate mirroring the dissent gate: reject silent removal of an unresolved risk; a resolution requires non-empty reason and non-empty provenance.
- [ ] 1.3 Update `build_respondent_prompt` in `deliberation.rs` to list unresolved risks (those with `resolved_by.is_none()`) by description.
- [ ] 1.4 Test: an unresolved risk carried forward keeps its id; silent removal is rejected; resolution without reason/provenance is rejected; valid resolution with provenance is accepted.

## 2. Risk persistence (build from scratch, mirror dissents)

- [ ] 2.1 In `store.rs::init`, add a `risks` table (id, revision_id, description, resolved_reason) and a `risk_resolution_provenance` table (risk_id, event_id).
- [ ] 2.2 In `commit_successor_revision`, verify every event referenced by a risk resolution exists, then insert risks and their provenance (mirroring dissents).
- [ ] 2.3 In `get_latest_revision`, load risks and their resolutions/provenance into `Revision.risks` instead of returning an empty vec.
- [ ] 2.4 Test: a revision with an open risk and a resolved risk round-trips through commit + reload with ids, reason, and provenance intact.

## 3. The convergence seam (compose suunta)

- [ ] 3.1 Add `crates/ringi/src/convergence.rs` and register it in `lib.rs`; import suunta types only here.
- [ ] 3.2 Map a `&Revision` to a `Bearing<()>` of `Correction`s — one per dissent and one per risk, `Sigil::new("dissent:<id>" | "risk:<id>")`, `Reversibility::Reversible`, body `()` — and a `Fix` with one `SatisfactionFinding` per target (`Satisfied` iff `resolved_by.is_some()`, else `Unsatisfied`).
- [ ] 3.3 Expose `is_ready(&Revision) -> bool` = `plan_residual(bearing, Sounding::new(fix, vec![])).is_converged()`.
- [ ] 3.4 Test: empty residual → true; one `Unsatisfied` target → false; a satisfied target is reported (not omitted) and a fully-resolved dossier → true; a directly-constructed `Sounding` with one `Unknown` finding → false.

## 4. Remove agent-authored and stored readiness

- [ ] 4.1 Remove `readiness` from `ArbitrationOutput` and from `Revision`.
- [ ] 4.2 Update `apply_arbitration` to stop copying readiness; adjust its return type to drop the readiness bool.
- [ ] 4.3 In `store.rs`, drop the `readiness` column from the revisions table schema, the select in `get_latest_revision`, and the insert in `commit_successor_revision`.
- [ ] 4.4 Update `dossier_cli.rs`: the initial revision no longer sets readiness; `inspect_command` prints `convergence::is_ready(&rev)` instead of a stored field.
- [ ] 4.5 Update all fixtures/tests that set `readiness` or build `unresolved_risks` as strings.

## 5. Loop drives the transition from suunta

- [ ] 5.1 In `deliberate_loop.rs`, transition `Submitted -> Deliberating` at start and persist.
- [ ] 5.2 Evaluate `convergence::is_ready` on the initial revision before the turn loop and transition to `ReadyForDecision` if already converged.
- [ ] 5.3 After committing each successor, evaluate `is_ready` on it and transition to `ReadyForDecision` (and break) on convergence — so final-turn convergence transitions.
- [ ] 5.4 Test: a dossier with `max_turns = 1` that converges after its single turn ends in `ReadyForDecision`, not `Deliberating`.

## 6. Keep the loop runnable (labelled temporary stopgap)

- [ ] 6.1 Add one clearly-labelled temporary line to `build_arbitrator_prompt` instructing the arbitrator to emit its successor-revision as exactly one line of compact JSON with no surrounding prose; comment it as owned by, and to be deleted with, the future `Motion` slice.
- [ ] 6.2 Test/verify: an end-to-end fixture run of the loop (fixture arbitrator emitting compact single-line JSON) completes a turn and reaches `ReadyForDecision` via suunta.

## 7. Definition of Done

- [ ] 7.1 Run the full DoD from `AGENTS.md` (build, test, clippy, fmt, doc, deny, naming-guard) and the end-to-end fixture run; report any command that cannot run in this environment.
