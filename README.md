# Ringi

Ringi is a local deliberation application for Agent CLIs. It takes one proposal through a bounded
稟議 process: respondents answer questions, an independent arbitrator maintains a durable dossier,
and a human records the final decision.

```text
draft → submit → answer → arbitrate → decide → archive
```

Agent CLIs are opaque respondents. Ringi supplies bounded natural-language context on stdin and
records their stdout answers; it does not govern whether they use OpenSpec or any other internal
method. Ringi itself performs no workspace mutation, patch application, or downstream execution.

## What ringi composes

- **pacta** for durable invocation lifecycle and recovery when honestly required;
- **suunta** for residual convergence over unresolved questions, dissent, risks, and conditions;
- **shaahid** for exactly-once recovery of one fixed Agent invocation.

Ringi owns dossier revisions, provenance, human decisions, archive rendering, and the thin wiring
between those concerns.

## Status

The project is being reframed from a code-work executor to the deliberation-only dossier model.
The active OpenSpec change is `reframe-ringi-deliberation`; the existing execution CLI is not the
future product contract.

## Architecture

- `PROJECT.md` — vision, invariants, and non-goals.
- `AGENTS.md` — operating protocol and Definition of Done.
- `BACKLOG.md` — recorded decisions and deferred work.
- `openspec/specs/` — shipped requirements.

## License

Licensed under either of [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT), at your option.
