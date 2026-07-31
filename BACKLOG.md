# Backlog & Deferred Decisions

Shipped truth lives in `openspec/specs/`; active proposed truth lives in `openspec/changes/`.
This file records product decisions, deferred work, and the family stance behind them.

## Completed Reframe

`reframe-ringi-deliberation` changed ringi from a code-work executor into a deliberation
application. Its unit of automation is one dossier: a human drafts and submits a proposal, Agent
CLIs answer bounded questions synchronously, an independent arbitrator maintains the current SSOT,
and a human concludes the dossier with an immutable archive. The change is complete: shipped
requirements live in `openspec/specs/`, and every capability below reflects the dossier domain, not
the deleted execution model.

The prior Builder/Reviewer/Verify loop proved the family-composition bet. It is historical
evidence, not the current product shape; the reframe removed workspace execution semantics rather
than relabelling them.

## Settled Decisions

- **Agent boundary:** stock Agent CLIs are opaque. Ringi uses stdin/stdout natural-language
  deliberation plus process metadata; OpenSpec is an optional internal method, not a contract.
- **Dossier truth:** raw prompts, answers, and judgments are append-only events. The latest complete
  public dossier revision is the sole respondent context and durable SSOT.
- **Synchronous MVP:** only one invocation is active for a dossier. This avoids stale answers,
  cancellation, merge barriers, and unforced in-flight semantics.
- **Spine and leaves:** respondents only answer; a logically separate arbitrator proposes the
  successor *transition*, never the decision; ringi holds and validates the canonical revision and
  never infers state from prose. The agent does not author the whole successor state (mechanism —
  `Motion` — see below).
- **Structured-move authorship (`Motion`) has shipped (2026-07-28):** the arbitrator no longer
  authors an entire successor `Revision` each turn. `ArbitrationOutput` is now
  `{ current_understanding, moves: Vec<Move> }`; `Move` covers resolving a dissent, adding or
  closing a risk, and asking or answering a question, each targeting one residual item by stable
  id with whatever provenance that kind requires. `Revision::apply_moves` validates each move
  individually and applies a batch atomically — one invalid move rejects the whole batch, matching
  the prior all-or-nothing turn behavior. `original_proposal`/`revision_id`/`parent_digest`/
  `content_digest` are no longer read from the agent at all, which removes the
  immutability-check bug class structurally rather than validating it more strictly. `Question`
  joined `Dissent`/`Risk` as a real residual item with its own convergence target category and
  store persistence; `build_respondent_prompt` gained an `## Open Questions` section, and
  `build_arbitrator_prompt` now shows every unresolved item's stable id (a gap found while
  dogfooding: ringi mints ids for new risks/questions, so without this the arbitrator could never
  target an existing item again). Conditions (the isolated per-condition evaluator loop) remain
  explicitly untouched — see `Residual expansion` below.
- **Convergence is mechanical, not agent-declared.** Readiness for human decision is computed by
  suunta (`plan_residual(...).is_converged()`) over the residual, never asserted by the arbitrator;
  readiness ceases to be an agent output. An `Unknown` verdict is conservatively retained, so
  unknown is never convergence.
- **Dissent:** unresolved dissent remains unless resolution includes a reason and source events;
  it can reopen on later evidence.
- **Arbitration policy:** users choose Economy, Balanced, or Assurance and may inspect advanced
  fixed settings. Submission locks the resolved policy, limits, and role bindings.
- **Session topology:** persistent arbitration is a cost optimization, never hidden truth. Fresh
  sessions reconstruct from the durable SSOT; Balanced and Assurance use them at locked boundaries.
- **Conditions:** `approve_with_conditions` is non-terminal. Human-authored conditions return to
  the residual and isolated evaluators answer true/false/unknown. All true returns to a human for
  final approval.
- **Sealed evaluation:** evaluator reasons are archived for humans but never injected into
  respondent or synthesis context. Evaluators verify; they do not coach.
- **Invalidation:** a human who judges arbitration untrustworthy invalidates the dossier. There is
  no in-place verdict override.
- **Archive:** approval produces a human-readable, integrity-bound record only. It grants no
  execution authority and triggers no workspace effect.

## Family Dependency Stance

Pacta, suunta, and shaahid remain published sibling mechanisms. Ringi retains each only if the new
domain exercises its public contract honestly:

- **pacta owns claim/settle/reclaim of a durable Agent invocation.** `registry.rs`'s
  `SqliteRegistry` claims a pact before invoking a respondent, arbitrator, or condition-evaluator,
  and settles it after — fulfilled on success, released for retry on failure — so a crash between
  invoking the agent and committing its result is durably distinguishable from a completed attempt.
  Revived from a pre-reframe implementation this repo had already conformance-proven, deleted as
  collateral with the old execution model rather than for being unfit.
- **suunta owns evaluation of the residual of dissents, risks, and questions.** `convergence.rs`
  projects a revision's open dissents, risks, and (since `Motion` shipped) questions onto a suunta
  `Bearing` and reports readiness from `Residual::is_converged()`; readiness is never an agent
  claim.
- **shaahid is assessed and deferred, not merely unattached — re-assessed after `Motion` shipped,
  Defer still holds.** `InvocationCoordinate::input_digest` is still a real content hash — now
  covering `original_proposal`, `current_understanding`, `positions`, `dissents`, `risks`, and
  `questions` — computed from whatever revision content the invocation's prompt was actually built
  from, exactly as before `Motion`. Coordinate identity and content remain the same value; shaahid's
  `Seal`/`Fingerprint` contradiction detection still has no gap to fill. The retry-drift failure
  mode it exists to catch is handled structurally: a coordinate reclaimed after release recomputes
  its `input_digest` from the current revision, so a retry against genuinely different content
  produces a genuinely different coordinate. **Re-open this only if a future coordinate scheme
  deliberately goes coarser than a full content hash** (e.g. for `Prompt-width granularity`'s narrow
  variant, not yet designed) — do not assume Defer survives a coordinate-scheme change made without
  revisiting this.

No dependency is retained for historical loyalty. Ringi must not recreate any retained mechanism.

suunta 0.1.1's shipped contract already covers ringi's convergence need in full: residual targets by
`Sigil`, a per-target `Satisfaction` verdict (including a conservative `Unknown`), `plan_residual`,
and `Residual::is_converged`. Ringi supplies the *verdict* — whether a dissent, risk, question, or
condition is satisfied — as the domain "verb" suunta deliberately keeps downstream; ringi must not
push suunta's contract to absorb that judgment. The only suunta seam ringi could legitimately force
later is coverage *production*, and only once parallel/in-flight deliberation makes ringi a real
coverage consumer. Any such advance happens in suunta's own repo, never inside a ringi change.

## Deferred Work

- **Prompt-width granularity:** `Motion` shipped the *wide* shape by construction — the arbitrator
  still declares its whole move batch in one invocation per turn, unchanged cardinality. A *narrow*
  variant (ringi enumerates the residual and asks one closed question per item, one invocation
  each) remains a real, undesigned alternative; wire it to the Economy/Balanced/Assurance posture
  rather than choosing globally, if it's ever pursued.
- **Residual expansion — conditions only now:** dissents, risks, and (since `Motion` shipped)
  questions are all real `Revision` residual items with stable ids and their own convergence
  target category. Conditions remain the one category still dossier-level and transient relative
  to the revision, evaluated by the separate isolated `ConditionEvaluator` mechanism — deliberately
  out of `Motion`'s scope (see Settled Decisions above). Folding conditions into the same residual
  model `Motion` now covers for the other three categories is the one piece of this old entry still
  open.
- **Executor consumer:** sandboxing, repository editing, verification commands, patch application,
  and any consumer of an approved archive require a separate change. They are not hidden inside
  this deliberation MVP.
- **Parallel deliberation:** blind parallel respondents, in-flight coverage, cancellation, and
  async scheduling remain deferred until latency or independence needs force them.
- **Family candidates:** Freigabe, Dychwel, and Stoma remain unforced. A sequential dossier has no
  demonstrated dependency-readiness, compensation, or circuit-posture requirement.
- **Cross-dossier reuse:** strategy migration and deriving a new dossier from an old archive are
  outside the one-dossier MVP.
- **Direct API adapter:** a thin, single configured model adapter remains possible only when forced;
  no routing, semantic caching, or provider layer is permitted.
