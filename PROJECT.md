# Project Contract

## Vision

Ringi is a local deliberation application for Agent CLIs. A *ringi* (稟議) is a proposal
circulated for challenge and approval before it acts. One ringi is a bounded dossier: agents answer
questions in natural language, an independent arbitrator maintains its current understanding, and a
human records the final decision in an auditable archive.

Ringi does not execute an approved decision. It does not edit a workspace, run repository checks,
apply patches, or operate OpenSpec. Those may become consumers of an approved dossier only when a
separate product need forces their shape.

## Positioning: a consumer that composes, not a monolith

Ringi is an application leaf. It owns its Agent-CLI deliberation domain and composes published
family primitives only where they honestly own a mechanic:

```text
durable invocation lifecycle + recovery -> pacta
residual over open questions and risks  -> suunta
exactly-once invocation recovery        -> shaahid
roles, dossier projections, provenance,
human decisions, and archive            -> ringi
```

The application MUST NOT recreate lifecycle, convergence, or idempotency mechanisms. It also MUST
NOT grow a provider layer, prompt-template platform, generic policy DSL, central framework, or
downstream executor.

## Core Contract

- **One dossier is the automation boundary.** Draft settings are mutable; submission locks them;
  a terminal human decision closes the dossier.
- **The model has no decision authority.** Respondents answer, arbitrators synthesize, and
  evaluators judge fixed conditions. Only a human approves, rejects, conditionally approves,
  cancels, or invalidates.
- **The current dossier revision is the work SSOT.** Raw events are append-only evidence, not
  later respondent context. A successor revision replaces the complete public working state.
- **Dissent is conservative.** It remains open unless an arbitrator gives a provenance-bound
  resolution; later evidence may reopen it. Unknown is never success.
- **Evaluator rationale is sealed.** It is archived for human audit but is mechanically excluded
  from all respondent and synthesis contexts.
- **Strategy is explicit and locked.** Economy, Balanced, and Assurance describe cost/quality
  postures; resolved session policy and limits are captured in submitted frontmatter.
- **Mechanical facts outrank NLP claims.** Process outcomes, durable links, digests, locks, and
  state transitions are enforced by ringi, not inferred from agent prose.
- **Limits are mandatory.** Turns, timeouts, cost, and strategy triggers bound every dossier.

## Terminology

`Dossier`, `Draft`, `Revision`, `Respondent`, `Arbitrator`, `Evaluator`, `Dissent`, `Risk`,
`Condition`, `Decision`, `Archive`, and `Controller` are ringi's native terms. Family terms remain
only at thin seams: `Pact`/`Registry`, `Bearing`/`Course`, and `Deed`/`witness`.

## Non-Goals

Ringi is not a workspace executor, patch applicator, sandbox platform, OpenSpec runtime, LLM
provider layer, MCP server, terminal emulator, distributed workflow engine, message broker,
Kubernetes worker pool, auto-deployer, or unrestricted shell. It does not run respondents in
parallel, manufacture in-flight coverage, or force Freigabe, Dychwel, or Stoma before a real
consumer need exists.

## References

- Naming worldview: `docs/naming.md`
- Operating protocol and Definition of Done: `AGENTS.md`
- Product decisions and deferred work: `BACKLOG.md`
- Shipped requirements: `openspec/specs/`
