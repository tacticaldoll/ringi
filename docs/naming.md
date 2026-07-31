# Naming worldview

Ringi is an application that **composes** three sibling libraries. It is uniquely exposed to
a semantic pull — queue-runtime speak — that, if adopted, drags a thin composer back toward
the heavy monolith ringi exists not to be. **Semantic drift precedes architectural drift.**
This document is the guardrail; `scripts/naming-guard.sh` mechanizes the hard part.

## Native register — ringi's own domain

Ringi's own domain names follow a clear **deliberative-governance** register — the arc a
*ringi* (稟議) runs: **draft → deliberate → decide → archive**. Ringi is an
application, not a published contract, so **clarity outranks evocativeness**: prefer a plain,
precise deliberative word over a cute metaphor.

Current native terms (keep them clear): `Dossier`, `Draft`, `Revision`, `Respondent`,
`Arbitrator`, `Evaluator`, `Dissent`, `Risk`, `Condition`, `Decision`, and `Archive`. The CLI's
`Command` is standard CLI vocabulary, not a domain concept — it is fine and exempt from the guard.

## Seam rule — brick terms stay at the seam

Each brick has its own register: pacta is legal/contract (`Pact`, `Registry`, `release`),
suunta is navigation (`Bearing`, `Course`, `Sigil`), shaahid is witness (`Deed`, `witness`).
When ringi calls a brick, it uses that brick's term — **but only in thin seam adapters**.
Brick vocabulary **must not** bleed into ringi's own
domain types or modules. The seam is where the registers meet, and it is bounded there.

## Banned — the queue-runtime / CQRS pull

These words **must not** name a ringi domain type, module, or trait:

> workflow · job · queue · worker · broker · dispatcher · dispatch · pipeline · scheduler ·
> runner · tenant · (and CQRS: command-as-a-domain-concept, event-handler, message-bus)

They are the mirrorlane/worklane gravity. A `WorkflowEngine`, a `JobQueue`, a
`Dispatcher` — each is the first visible sign of re-monolithing. If you reach for one, stop:
the hard mechanics are the bricks' (durable lifecycle → pacta, convergence → suunta,
idempotency → shaahid). Naming one of them yourself is the monolith returning.

Note the handoff spec that seeded ringi drifts here ("workflow orchestrator", a
"dispatcher/broker" interface); those are to be renamed to ringi's register, not adopted.

## Enforcement

`scripts/naming-guard.sh` fails if a banned word names a `struct`/`enum`/`trait`/`type`/`mod`
in the Rust sources; it runs in the Definition of Done (`AGENTS.md`). It is deliberately
high-precision (declarations only) so it does not false-positive on prose or CLI vocabulary;
the soft cases above stay review-governed.
