# Project Contract

## Vision

Ringi is a local automation orchestrator for Agent CLIs. A *ringi* (稟議) is a proposal
circulated for review and approval before it acts; ringi runs exactly that shape over code
work: a **Builder** agent proposes changes, a **Reviewer** agent scrutinizes them, objective
tools verify them, policy and a human gate anything risky, and the loop retries until the
work converges or is stopped.

Ringi does **not** think or edit — the Agent CLIs do. Ringi is the thin controller that owns
sequencing, durable state, verification, permission, and stop conditions. Crucially, it does
not reimplement the hard mechanics: it **composes** them from published family primitives.

## Positioning: a consumer that composes, not a monolith

Two predecessors died as domain-driven monoliths (a rich `Run`/`Step` engine that owned
everything). Ringi refuses that shape. The hard parts come from bricks; ringi owns only its
own blood — the Agent-CLI domain — and the wiring loop.

```text
durable step lifecycle + recovery   -> pacta   (Registry contract; ringi's own SQLite backend)
convergence / "is this run done?"    -> suunta  (residual over open review issues + failed checks)
step idempotency / resume safety     -> shaahid (witness a step attempt; do it exactly once)
authorization / approval gating      -> ringi consumer code now; an authorization-gate brick may extract later
Agent-CLI adapters, subprocess, git worktree, policy content, prompts, artifacts, the loop
                                      -> ringi's own blood
```

Ringi is a **leaf**: it depends on the family; nothing depends on it. It is an *application*,
governed as one (security, config, versioning) — not a sans-I/O library brick.

## Core Contract

The invariants to protect first:

- **The model has no execution authority.** An agent only *proposes* actions; ringi and its
  policy engine decide what actually runs. Reviewer approval is a *quality* opinion, never a
  *permission*.
- **Tool verification outranks model opinion.** Completion is decided by objective checks
  (tests, lint, build) that ringi re-runs itself, never by an agent's claim of success.
- **The store is the source of truth.** Durable run/step state, an event log, and artifacts
  survive restarts; a run resumes from its last durable checkpoint.
- **Limits are mandatory.** Max rounds, timeouts, max diff/files/log, workspace boundary,
  and a deny-list of high-risk actions bound every run.
- **Compose, do not reimplement.** Lifecycle, convergence, and idempotency are the family's;
  ringi wires them. Reimplementing any of them in ringi is the monolith returning.

## Terminology

`Run`, `Step` (Build / Review / Verify / Approval), `Builder`, `Reviewer`, `Controller`
(ringi itself, the final judge), `Action`, `Policy` (Allow / Ask / Deny), `Approval`,
`Verification`. Family terms are used as-is at the seams (`Pact`/`Registry`/`release`,
`Bearing`/`Course`/`is_converged`, `Deed`/`witness`).

## Non-Goals

Ringi is not: its own LLM API layer, an MCP server, a terminal emulator, a distributed
multi-host workflow, a message broker, a Kubernetes worker pool, an auto-deployer, or an
unrestricted shell. Agent CLIs bring the intelligence and tools; ringi brings order, proof,
permission, and durability.

On the LLM-API line specifically — the non-goal is the **layer**, not the act of calling a
model. `AgentAdapter` is the abstraction over *how an agent is invoked*; a `SubprocessAdapter`
(Agent-CLI) externalizes model choice, prompts, keys, and caching into the CLI, which is why it
keeps ringi thin. A direct-API adapter is a legitimate second branch of that same seam **iff it
stays thin**: a single config-specified model, retry/gating **borrowed from composed Layers**
(see `docs/round-model.md` ③), and **no caching**. What is forbidden is the *provider layer* —
model routing/arbitration, semantic caching, prompt-template management — i.e. what a
predecessor (`../mirrorlane`) grew as its `provider` crate. Agent CLIs are the default; a
direct-API adapter is the thin exception, never a provider.

## References

- Naming worldview (native register, seam rule, banned queue-runtime vocabulary): `docs/naming.md`
- Operating protocol and Definition of Done: `AGENTS.md`
- Phased plan, deferred decisions, and the family-dependency stance: `BACKLOG.md`
- Shipped requirements: `openspec/specs/`
