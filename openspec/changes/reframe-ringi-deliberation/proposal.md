## Why

Ringi's current build-review-verify controller proves that the family bricks compose, but it
places code execution at the center of a product whose name and strongest differentiated value are
deliberation and accountable human decision. Reframe the application around one bounded *ringi*:
Agent CLIs answer and challenge a proposal through natural language, an independent arbitrator
maintains a durable single source of truth, and a human closes the case with an auditable decision.

## What Changes

- **BREAKING** Replace the workspace-editing run contract with a deliberation contract whose unit
  of automation is one dossier from draft through archived human decision.
- Treat an Agent CLI invocation as an opaque respondent: ringi supplies natural-language context
  on stdin and consumes its natural-language answer from stdout without governing the agent's
  internal method (including whether it uses OpenSpec).
- Advance deliberation synchronously, one invocation at a time, through respondent answers and an
  independent arbitration session that proposes complete successor revisions of the dossier SSOT.
- Preserve raw answers and arbitration records as append-only events while exposing only the
  current dossier revision to later respondent sessions.
- Introduce user-selectable economy, balanced, and assurance arbitration strategies, with
  inspectable advanced settings for persistent versus fresh arbitration sessions and review
  triggers. Lock the resolved strategy and limits when a draft is submitted.
- Conservatively retain dissent and unresolved risks. Automated resolution requires a reason and
  provenance; arbitration reasons are archived for humans but are mechanically excluded from all
  respondent contexts.
- Add human decisions for approve, reject, approve-with-conditions, cancel, and invalidate.
  Conditions re-enter the same deliberation as fixed predicates evaluated independently as true,
  false, or unknown; only a later human approval closes the case as approved.
- Produce an immutable, human-readable archive with identity, revisions, digests, provenance,
  sealed arbitration records, and the final decision. The MVP performs no workspace mutation or
  downstream execution.
- Update the project vision, backlog, naming guidance, CLI, persistence model, and tests to match
  the deliberation-only boundary. Retain pacta, suunta, and shaahid only where they compose honest
  lifecycle, residual, and exactly-once mechanics; do not manufacture async or in-flight work.

## Capabilities

### New Capabilities

- `deliberation-dossier`: Human-readable dossier drafts, locked submission settings, immutable
  revisions, and the current SSOT projection.
- `deliberation-loop`: Synchronous respondent/arbitrator turns that advance questions, dissent,
  unresolved risks, and decision readiness without exposing sealed arbitration feedback.
- `arbitration-strategy`: Economy, balanced, and assurance policies for persistent and fresh
  arbitration sessions, including inspectable resolved settings and bounded escalation triggers.
- `human-decision`: Human approve, reject, approve-with-conditions, cancel, and invalidate
  decisions, including condition predicates that re-enter deliberation before final approval.
- `dossier-archive`: Immutable archival rendering of the proposal, revisions, public event
  history, sealed arbitration justifications, provenance, integrity digests, and final decision.
- `dossier-cli`: Deliberative commands for drafting, submitting, continuing, inspecting, deciding,
  and archiving one dossier.
- `durable-dossiers`: SQLite persistence and atomic provenance/revision commits for dossiers.
- `resumable-dossiers`: Crash-safe continuation from durable dossier state without hidden session
  state or duplicate Agent CLI calls.
- `dossier-assembly`: Composition of respondent, arbitration, evaluator, strategy, and persistence
  roles from locked dossier settings.

### Modified Capabilities

- `agent-adapter`: Narrow the common adapter result to process outcome plus natural-language stdout;
  provider-specific structured envelopes remain optional transport metadata.
- `cli-run`: Remove workspace task execution commands and configuration.
- `durable-runs`: Remove persisted workspace code-run semantics.
- `reconcile-loop`: Remove the Builder/Reviewer/Verify reconcile contract.
- `resumable-runs`: Remove code-run checkpoint and resume semantics.
- `run-assembly`: Remove Builder, Reviewer, command verification, and workspace assembly.
- `builder-execution`: Remove the Builder/workspace-editing contract.
- `reviewer-execution`: Remove the code-review findings contract.
- `verification`: Remove command verification as a completion authority.

## Impact

- Rewrites `PROJECT.md`, `BACKLOG.md`, relevant architecture/naming documentation, and most living
  product requirements.
- Replaces the current `run`, Builder/Reviewer, verification, workspace, and persisted run surfaces
  in `crates/ringi` with dossier, turn, arbitration, decision, and archive surfaces.
- Changes the CLI and on-disk SQLite schema; compatibility with pre-deliberation run records and
  commands is not promised before the first release.
- Keeps ringi an application leaf and preserves the compose-do-not-reimplement boundary. Published
  family dependencies remain sibling mechanisms, not a central framework; whether each remains is
  decided by demonstrated use in the new loop.
- Does not introduce an LLM provider layer, workspace executor, sandbox platform, OpenSpec runtime,
  async scheduler, or downstream Apply capability.
