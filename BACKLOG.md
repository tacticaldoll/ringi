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
- **Spine and leaves:** respondents only answer; a logically separate arbitrator proposes complete
  successor revisions; ringi persists the transition without interpreting prose.
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

## Deferred Work

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
