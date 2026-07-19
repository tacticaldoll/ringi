# Ringi

Ringi is a local automation orchestrator for Agent CLIs. It drives a
**Builder → Reviewer → verify → gate → retry** loop over code work, keeping durable state,
enforcing policy and human approval, and verifying with objective tools — while the Agent
CLIs do the thinking and editing.

A *ringi* (稟議) is a proposal circulated for review and approval before it acts. Ringi runs
that shape: agents propose, tools and a reviewer scrutinize, policy and a human gate, and the
loop converges or stops.

## What ringi owns — and what it composes

Ringi is a thin controller, not a runtime. The hard mechanics come from the pacta family:

- **pacta** — durable step lifecycle (claim → execute → settle, with `release` for retry);
- **suunta** — convergence: is the run done?
- **shaahid** — step idempotency, so a resumed step never runs twice.

Ringi owns only its own domain: Agent-CLI adapters, subprocess execution, git-worktree
isolation, policy content, approval, artifacts, and the loop that wires it all. It is a
family **leaf** — an application, not a library.

## Status (0.1.0, in development)

Project shape only: the command surface exists (stubbed); behavior is built bet-first — see
`BACKLOG.md`. The first milestone is a minimal composition loop proving the family bricks
compose with acceptable friction.

## Architecture

- `PROJECT.md` — vision, the invariants to protect, non-goals.
- `AGENTS.md` — operating protocol and the Definition of Done.
- `BACKLOG.md` — the bet, the phased plan, seam design, and the family-dependency stance.
- `openspec/specs/` — shipped requirements.

## License

Licensed under either of [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT), at your option.
