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
- **Structured-move authorship (`Motion`) is decided, not deferred (2026-07-28):** the arbitrator
  authoring an entire successor `Revision` each turn, rather than declaring discrete moves, is
  promoted from Deferred Work to a committed direction. Evidence: a real bug this session
  (`fix(revision): enforce original_proposal immutability across successors`) where the
  whole-successor JSON silently mutated a field that must never change, caught only by a post-hoc
  diff — plus the `TEMPORARY STOPGAP` comment already naming the one-line-JSON coupling in
  `build_arbitrator_prompt` as fragile and destined for removal. This decision is scoped to the
  authorship mechanism itself: it does **not** also settle `Residual expansion` or `Prompt-width
  granularity` (still Deferred Work below, now unlocked for design rather than parked) or shaahid's
  re-assessment (Family Dependency Stance below) — those need their own design/evaluation work
  before they are themselves decided. `Motion`'s own data model, invocation shape, and
  apply/validate mechanics are not designed yet; this entry records that ringi has decided to do
  that design work, not what it will conclude.
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
- **suunta owns evaluation of the residual of dissents and risks.** `convergence.rs` projects a
  revision's open dissents and risks onto a suunta `Bearing` and reports readiness from
  `Residual::is_converged()`; readiness is never an agent claim.
- **shaahid is assessed and deferred, not merely unattached.** A structural-dependency assessment
  concluded **Defer**: `InvocationCoordinate::input_digest` is already `Revision::content_digest`,
  computed over exactly the fields (`original_proposal`, `current_understanding`, `positions`,
  `dissents`, `risks`) that `build_respondent_prompt` reads — so a coordinate's identity already
  ties to its content, and shaahid's `Seal`/`Fingerprint` contradiction detection (a drifted
  `Fingerprint` under a repeated `Seal`, or the reverse) has no path to fire today. The reopening
  trigger was **Motion** or **prompt-width granularity** letting an invocation's actual content vary
  independently of those five fields — **Motion is now a Settled Decision (2026-07-28, see
  above)**, so this trigger has fired in principle. Re-running the assessment is still blocked on
  `Motion`'s own design landing (its actual invocation-content shape isn't defined yet); do not
  assume the prior Defer still holds once that design exists, and do not re-run the assessment
  before it does — there is nothing concrete yet to assess against.

No dependency is retained for historical loyalty. Ringi must not recreate any retained mechanism.

suunta 0.1.1's shipped contract already covers ringi's convergence need in full: residual targets by
`Sigil`, a per-target `Satisfaction` verdict (including a conservative `Unknown`), `plan_residual`,
and `Residual::is_converged`. Ringi supplies the *verdict* — whether a dissent, risk, question, or
condition is satisfied — as the domain "verb" suunta deliberately keeps downstream; ringi must not
push suunta's contract to absorb that judgment. The only suunta seam ringi could legitimately force
later is coverage *production*, and only once parallel/in-flight deliberation makes ringi a real
coverage consumer. Any such advance happens in suunta's own repo, never inside a ringi change.

## Deferred Work

- **Prompt-width granularity:** unlocked (not settled) by `Motion` being decided (Settled
  Decisions, above) — the same `Motion` substrate would admit a wide prompt (the agent enumerates
  and declares many moves in one call) or a narrow one (ringi enumerates the residual and asks one
  closed question per item). This is prompt width and invocation count, not two architectures;
  wire it to the Economy/Balanced/Assurance posture rather than choosing globally, once `Motion`'s
  own design exists to wire it against.
- **Residual expansion:** unlocked (not settled) by `Motion` being decided — for suunta's residual
  to cover all four categories, open questions and conditions must live in the `Revision` rather
  than be transient, and risks need stable ids (they are bare strings today) so each residual item
  carries a `Sigil`. v1 convergence counts dissents and risks only; questions and conditions follow
  once `Motion`'s design defines how they're declared and applied.
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
