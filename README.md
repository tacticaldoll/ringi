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

- **pacta** for durable invocation lifecycle and recovery — claiming and settling every
  respondent, arbitrator, and condition-evaluator invocation, so a crash between invoking an Agent
  CLI and committing its result is durably distinguishable from a completed attempt.
- **suunta** for mechanical convergence over the residual — dissents, risks, questions, all
  projected onto a suunta `Bearing`; readiness for a human decision is a mechanical fact, never an
  agent claim.
- **cadw** for atomic batch-fold validation of a `Move`/`ConditionMove` batch — existence,
  state-machine, and duplicate-target rules over addressable residual targets, composed via the
  `residual_ledger` seam.

**shaahid was assessed and declined**, not composed: `InvocationCoordinate`'s own content-derived
identity already closes the identity/content-drift gap shaahid exists to fill, so there is no gap
left for it — see `BACKLOG.md`'s Family Dependency Stance for the full reasoning.

Ringi owns dossier revisions, provenance, human decisions, archive rendering, and the thin wiring
between those concerns.

## The deliberation model

An arbitration turn never authors a whole successor revision. It declares zero or more discrete,
provenance-bound `Move`s — add or resolve a dissent, add or close a risk, ask or answer a question
— which ringi validates and applies atomically (`Motion`). Conditions (human-added once a dossier
reaches `ReadyForDecision`, judged by an isolated `ConditionEvaluator`) are a fourth residual
category, mutated through the analogous `ConditionMove`, deliberately kept out of the arbitrator's
own vocabulary — an arbitrator can never author or satisfy a condition.

A dossier's terminal archive is a human-readable, integrity-bound record only: every dissent,
risk, question, and condition with its final status, every recorded event (public claims and
sealed evaluator reasoning), and a SHA-256 digest over the rendered content. It grants no
execution authority and triggers no workspace effect — consuming that decision (executing it,
applying a patch, running verification) is explicitly out of scope; see `BACKLOG.md`'s Deferred
Work.

## Usage

```sh
ringi init                            # provision the local SQLite store
ringi draft                           # create a draft dossier
ringi submit <id>                     # lock settings, commit the initial revision
ringi continue <id>                   # run the deliberation turn loop
ringi condition <id> "<description>"  # add a condition (once ReadyForDecision)
ringi evaluate <id>                   # judge unmet conditions in isolation
ringi approve <id>                    # or: reject / cancel / invalidate
ringi reopen <id>                     # ApprovedWithConditions -> ReadyForDecision
ringi inspect <id>                    # current state, readiness, residual
```

`ringi <command> --help` lists every flag. A dossier's `respondent`/`arbitrator` roles are program
paths, locked at submit time — point them at any Agent CLI (including a thin wrapper script) that
reads a prompt on stdin and writes its answer to stdout.

## Architecture

- `PROJECT.md` — vision, invariants, and non-goals.
- `AGENTS.md` — operating protocol and Definition of Done.
- `BACKLOG.md` — recorded decisions and deferred work.
- `docs/naming.md` — the naming worldview and seam discipline.
- `openspec/specs/` — shipped requirements.
- `CHANGELOG.md` — release history.

## License

Licensed under either of [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT), at your option.
