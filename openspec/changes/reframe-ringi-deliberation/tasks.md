## 1. Product Contract

- [x] 1.1 Rewrite `PROJECT.md`, `README.md`, and `BACKLOG.md` around the deliberation-only dossier boundary, record the superseded execution vision as history rather than current behavior, and run the complete Definition of Done.
- [x] 1.2 Update naming and architecture documentation so dossier, respondent, arbitration, dissent, condition, decision, and archive vocabulary is explicit while workspace execution and unforced async family candidates remain non-goals; run the complete Definition of Done.

## 2. Dossier Domain

- [x] 2.1 Add human-readable dossier/frontmatter types with draft editing, preset resolution, submission-time locking, lifecycle states, and strict parse/round-trip tests; run the complete Definition of Done.
- [x] 2.2 Add immutable complete SSOT revisions, parent/content digests, public residuals, dissent resolution/reopen provenance, and structural rejection tests for unsupported removal; run the complete Definition of Done.
- [x] 2.3 Add append-only public and sealed event types plus context projections proving raw transcripts and sealed evaluator records never enter respondent prompts; run the complete Definition of Done.

## 3. Durable Store And Recovery

- [x] 3.1 Replace pre-release run tables with dossier, locked-settings, revision, event, dissent, condition, decision, and sealed-evaluation tables in the one SQLite store; prove reopen persistence and non-destructive initialization for dossier data, then run the complete Definition of Done.
- [x] 3.2 Commit provenance events and successor revisions atomically, reject broken parent/event references, and test interruption without orphan state; run the complete Definition of Done.
- [x] 3.3 Define stable invocation coordinates over dossier, role, input revision or condition snapshot, turn, and attempt; compose honest pacta/shaahid recovery or remove ceremonial composition, prove no duplicate CLI call after lost settlement, and run the complete Definition of Done.

## 4. Agent CLI Roles

- [x] 4.1 Narrow `AgentAdapter` to process outcome, natural-language stdout answer, stderr, session instruction, and optional transport metadata while preserving no-shell invocation, minimized environment, timeout, and portability tests; run the complete Definition of Done.
- [x] 4.2 Implement respondent prompt construction from only the original proposal, current public revision, and unresolved items; prove respondent answers are claims that cannot mutate the SSOT, then run the complete Definition of Done.
- [x] 4.3 Implement logically separate arbitration sessions that propose complete successor revisions with readiness and next-question output; reject malformed or structurally invalid proposals and run the complete Definition of Done.
- [x] 4.4 Implement isolated condition evaluators returning true/false/unknown plus sealed justification and provenance; prove their prompts, reasons, and verdict records never reach respondent or synthesis context, then run the complete Definition of Done.

## 5. Arbitration Strategies

- [x] 5.1 Implement closed, inspectable economy, balanced, and assurance preset resolution with advanced fixed fields, validation, and submission snapshots; run the complete Definition of Done.
- [x] 5.2 Implement persistent arbitration session reuse and fresh-session reconstruction from durable SSOT without making session memory authoritative; run the complete Definition of Done.
- [x] 5.3 Implement balanced trigger escalation and assurance fresh-session granularity with deterministic strategy traces and cost/session provenance; run the complete Definition of Done.

## 6. Synchronous Deliberation

- [x] 6.1 Replace Builder/Reviewer/Verify rounds with a strictly single-invocation respondent → arbitration fold and tests proving the next invocation starts only after durable commit; run the complete Definition of Done.
- [x] 6.2 Map unresolved questions, dissent, risks, and fixed conditions to suunta residual semantics or remove suunta if the mapping is ceremonial; prove readiness stops automation without granting approval and run the complete Definition of Done.
- [x] 6.3 Implement conservative automatic dissent resolution and reopening with reason/provenance retention across revisions; run the complete Definition of Done.
- [x] 6.4 Implement durable continuation after process loss for each boundary between respondent, arbitration, revision commit, and readiness without relying on hidden session state; run the complete Definition of Done.

## 7. Human Decisions And Archive

- [ ] 7.1 Implement human approve, reject, cancel, and invalidate transitions with terminal-state enforcement and no verdict override path; run the complete Definition of Done.
- [ ] 7.2 Implement non-terminal approve-with-conditions, immutable predicates, repeated isolated evaluation, and return to human decision only after every condition is true; run the complete Definition of Done.
- [ ] 7.3 Render immutable human-readable archives containing proposal, final SSOT, resolved strategy, revisions, public event index, decisions, integrity digests, and a separately labelled sealed audit section; run tamper tests and the complete Definition of Done.
- [ ] 7.4 Prove final approval produces only an archive and performs no workspace edit, command verification, patch application, or downstream execution; run the complete Definition of Done.

## 8. CLI And End-To-End Flow

- [ ] 8.1 Replace the workspace run commands and config scaffold with deliberative CLI commands for draft, submit, continue, inspect, approve, condition, reject, cancel, and invalidate using naming-worldview-compliant vocabulary; run the complete Definition of Done.
- [ ] 8.2 Add end-to-end binary fixtures for economy, balanced, and assurance dossiers from draft through each human terminal decision, including restart and sealed-context exclusion; run the complete Definition of Done.
- [ ] 8.3 Remove obsolete Builder, workspace, command-verification, patch, and code-run modules/tests/configuration after their replacements are proven; verify no execution-language compatibility shim remains and run the complete Definition of Done.

## 9. Apply Review And Integration Readiness

- [ ] 9.1 Perform the required adversarial apply review: check for duplicated family mechanics, hidden model decision authority, a provider/policy monolith, mutable locked settings, dissent loss, sealed-feedback leakage, secret exposure, workspace effects, and unforced async/in-flight abstractions; resolve every finding and run the complete Definition of Done.
- [ ] 9.2 Reconcile documentation, code, and delta specs; run the complete Definition of Done, then mark the implementation ready for agent-driven spec sync and completed-change deletion.
