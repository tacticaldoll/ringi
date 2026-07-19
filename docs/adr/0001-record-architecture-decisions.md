# ADR 0001: Record Architecture Decisions

## Status

Accepted

## Context

Important project decisions should survive beyond the chat, issue, or pull
request where they were made. Architecture decision records provide a compact,
version-controlled way to preserve context and tradeoffs.

## Decision

Use lightweight ADRs under `docs/adr/` for significant technical and governance
decisions.

Each ADR should include:

- status
- context
- decision
- consequences

## Consequences

- New contributors and agents can recover why decisions were made.
- Reversing a decision requires recording the new decision rather than silently
  rewriting history.
- Small decisions do not need ADRs; avoid ceremony for routine implementation
  details.
