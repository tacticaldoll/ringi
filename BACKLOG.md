# Backlog & Deferred Decisions

Shipped truth lives in `openspec/specs/`; active proposed truth lives in `openspec/changes/`.
This file records product decisions, deferred work, and the family stance behind them.

## Current Reframe

`reframe-ringi-deliberation` changes ringi from a code-work executor into a deliberation
application. Its unit of automation is one dossier: a human drafts and submits a proposal, Agent
CLIs answer bounded questions synchronously, an independent arbitrator maintains the current SSOT,
and a human concludes the dossier with an immutable archive.

The existing Builder/Reviewer/Verify loop proved the family-composition bet. It is historical
evidence, not the future product shape. The active change removes workspace execution semantics
rather than relabelling them.

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
  `Motion` — under Deferred Work).
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

- pacta may own claim/settle/reclaim of a durable Agent invocation;
- shaahid may prevent a reclaimed fixed invocation from calling a CLI twice;
- suunta may evaluate the residual of questions, dissent, risks, and conditions.

No dependency is retained for historical loyalty. Ringi must not recreate any retained mechanism.

suunta 0.1.1's shipped contract already covers ringi's convergence need in full: residual targets by
`Sigil`, a per-target `Satisfaction` verdict (including a conservative `Unknown`), `plan_residual`,
and `Residual::is_converged`. Ringi supplies the *verdict* — whether a dissent, risk, question, or
condition is satisfied — as the domain "verb" suunta deliberately keeps downstream; ringi must not
push suunta's contract to absorb that judgment. The only suunta seam ringi could legitimately force
later is coverage *production*, and only once parallel/in-flight deliberation makes ringi a real
coverage consumer. Any such advance happens in suunta's own repo, never inside a ringi change.

## Deferred Work

- **Structured-move authorship (`Motion`):** replace the arbitrator authoring an entire successor
  `Revision` (and its JSON-or-crash coupling) with a ringi-native `Motion` — a discrete,
  provenance-bound, individually-validated operation on the residual (resolve dissent, add or close
  risk, answer question) that the agent *declares* and ringi applies. Absence of a declared move is
  a no-op, never an inference from prose. Until it lands, a labelled one-line stopgap keeps the
  arbitrator prompt emitting parseable output so the loop runs.
- **Prompt-width granularity:** the same `Motion` substrate admits a wide prompt (the agent
  enumerates and declares many moves in one call) or a narrow one (ringi enumerates the residual and
  asks one closed question per item). This is prompt width and invocation count, not two
  architectures; wire it to the Economy/Balanced/Assurance posture rather than choosing globally.
- **Residual expansion:** for suunta's residual to cover all four categories, open questions and
  conditions must live in the `Revision` rather than be transient, and risks need stable ids (they
  are bare strings today) so each residual item carries a `Sigil`. v1 convergence counts dissents
  and risks only; questions and conditions follow with `Motion`.
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
