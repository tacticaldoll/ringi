## Context

Ringi currently persists and drives a Builder → Reviewer → command-verification loop over a
workspace. That implementation proved the family composition bet, but it also made ringi a code
executor and pulled the product away from the deliberative meaning of *ringi*: a proposal is
circulated, challenged, completed, and decided before action.

This change keeps the useful mechanical seams—opaque Agent CLI invocation, durable lifecycle,
residual convergence, and exactly-once recovery—but changes the owned domain. One dossier is the
automation boundary. It begins as a human-authored draft, advances through synchronous natural-
language turns, and ends as an immutable archive of a human decision. The application does not
edit a workspace or execute an approved decision.

The design must preserve four constraints: models never grant approval; the durable store is the
source of truth; NLP is interpreted only by replaceable Agent CLI roles; and ringi must compose,
not recreate, pacta lifecycle, suunta convergence, or shaahid idempotency.

## Goals / Non-Goals

**Goals:**

- Make a dossier revision the durable SSOT for one deliberation while retaining every raw event.
- Let stock Agent CLIs participate through natural-language stdin/stdout without a ringi-specific
  internal workflow contract.
- Keep respondent, synthesis, and condition-evaluation authority separate.
- Offer inspectable economy, balanced, and assurance strategies in the MVP.
- Reduce human work through conservative automatic dissent resolution while reserving every final
  decision for a human.
- Produce a human-readable, integrity-bound archive that explains the decision without feeding
  sealed evaluator feedback back into respondents.

**Non-Goals:**

- Editing code, running workspace commands, applying patches, or consuming an approved dossier.
- Understanding or driving OpenSpec inside a respondent's repository.
- A provider router, prompt-template platform, semantic cache, generic policy DSL, or central
  middleware engine.
- Concurrent respondents, asynchronous planning, in-flight cancellation, dependency readiness,
  compensation, circuit breaking, or other family candidates not forced by the synchronous MVP.
- Cross-dossier inheritance, strategy migration within an active dossier, or compatibility with
  pre-release persisted code-run records.

## Decisions

### One submitted dossier is the immutable policy envelope

A Markdown dossier has machine-readable frontmatter and a natural-language body. During `draft`,
the user may edit strategy, limits, and role bindings. Submission resolves presets into explicit
settings and locks every user-controlled frontmatter field for the lifetime of that dossier.
Subsequent revisions replace the complete SSOT body and update only ringi-owned fields such as
revision, parent digest, status, and timestamps.

Alternative: allow strategy edits as new revisions. Rejected because a single dossier would no
longer represent one comparable cost/quality policy, and a persistent session could carry hidden
state across the apparent policy change.

### Events are history; the current revision is working truth

Every prompt, stdout answer, arbitration proposal, verdict, human decision, and state transition is
append-only. Later respondent sessions receive the original proposal plus the current public SSOT
and unresolved items—not the raw transcript. A transaction commits an event and its successor
revision together, so recovery never observes a revision without its provenance.

Alternative: replay the entire transcript into every session. Rejected because context grows
without bound, earlier answers anchor later respondents, and a hidden conversational history would
compete with the durable SSOT.

### Agent CLIs are opaque natural-language respondents

The common adapter supplies a prompt on stdin, captures stdout/stderr and process outcome, and
returns stdout as an answer. Claude/Codex JSON envelopes and session identifiers are adapter
metadata, not ringi domain semantics. Best-effort structured transport remains allowed, but a valid
natural-language answer needs no shared `AgentResult` schema.

Alternative: require every CLI to return a ringi-owned business schema. Rejected because it forces
unmodified Agent CLIs into ringi's worldview and recreates a provider abstraction.

### Deliberation is a synchronous spine–leaf fold

Only one Agent CLI invocation is active at a time. A respondent leaf answers one question and
cannot update the SSOT. A logically independent arbitration role reads the current revision plus
that answer and proposes a complete successor revision: current understanding, positions,
dissent, unresolved risks, readiness, and the next question. Ringi validates structural invariants
and atomically commits the proposal; it does not interpret the prose.

Alternative: parallel blind respondents. Deferred because no MVP requirement justifies stale
in-flight answers, merge barriers, cancellation, or coverage semantics. Concurrency must be forced
by observed latency or independence needs.

### Dissent retention is conservative and provenance-bound

An unresolved dissent remains in every successor revision unless the arbitration proposal records
an explicit resolution reason and references the answer events that support it. Resolution is a
state transition, not deletion; later evidence may reopen it. Ringi checks the presence and
referential integrity of the reason and provenance without judging their meaning.

Alternative: trust the arbitrator to rewrite the summary freely. Rejected because disagreements
could disappear silently and the archive could manufacture consensus.

### Strategies select session topology, not business semantics

The MVP exposes intent presets backed by explicit resolved settings:

- `economy`: one persistent arbitration session for ordinary synthesis and resolution.
- `balanced`: persistent arbitration with fresh-session review at configured confidence, severity,
  and pre-decision triggers.
- `assurance`: fresh arbitration at the configured round or resolution granularity.

Advanced users can inspect and override fixed enum/threshold fields before submission. Strategy
code decides when and where to request an NLP judgment; the arbitrator supplies the judgment. The
archive records the resolved policy and actual session topology used.

Alternative: one hard-coded strategy. Rejected because choosing a transparent cost/quality posture
is part of the MVP product contract, not a later implementation optimization.

### Conditional approval is a non-terminal human event

`approve-with-conditions` fixes human-authored predicates and returns the dossier to deliberation.
An isolated evaluator session receives one condition and the current answer/evidence snapshot and
returns `true`, `false`, or `unknown`, plus a concise justification and provenance. The controller
uses only the verdict. The reason is sealed audit material and is mechanically excluded from every
respondent and synthesis prompt, so the evaluator cannot coach a session to satisfy itself. All
conditions becoming true makes the dossier eligible for another human decision; it never grants
approval automatically.

Alternative: make conditional approval terminal and delegate fulfillment to a future consumer.
Rejected because the MVP boundary is one completed deliberation, not an executable mandate.

### Human invalidation terminates an untrustworthy proceeding

Humans may approve, reject, conditionally approve, cancel, or invalidate. If a human concludes that
arbitration was wrong, the dossier becomes terminally invalidated; the verdict is not overridden in
place. The archive preserves the failed process and reason. A replacement dossier is outside this
change.

### The archive is a document, not execution authority

The terminal artifact contains the original proposal, final public SSOT, revision/event index,
resolved strategy, human decisions, integrity digests, and a separately labelled sealed evaluation
section. The archive is for human inspection and future reference. `approved` has no built-in side
effect and grants no downstream process authority.

### Family mechanisms remain conditional on honest use

Pacta may own durable invocation claim/settle/reclaim, shaahid may prevent a reclaimed invocation
from calling an Agent CLI twice, and suunta may evaluate the residual of unresolved questions,
dissent, risks, and conditions. The implementation must first map the new domain onto their public
contracts and remove any composition that has become ceremonial. It must not introduce async,
coverage, Freigabe, Dychwel, or Stoma merely because the family roadmap contains them.

## Risks / Trade-offs

- **[Model synthesis can distort the SSOT]** → Preserve raw events, require reason/provenance for
  dissent resolution, isolate respondent and arbitration sessions, and leave final authority human.
- **[Persistent sessions create a hidden second truth]** → Inject the current revision every turn,
  make durable recovery possible from the revision alone, and use fresh review in stronger modes.
- **[Sealed reasons reduce respondents' ability to self-correct]** → Respondents see the original
  fixed condition and public SSOT; humans can inspect evaluator reasons, but evaluator coaching is a
  deliberate non-goal.
- **[Natural-language outputs are difficult to parse reliably]** → Keep the common answer opaque;
  ask arbitration/evaluation roles for bounded output shapes where an adapter supports them, and
  fail closed on missing control verdicts.
- **[A large reframe can preserve obsolete code by relabelling it]** → Remove Builder, workspace,
  command-verification, and patch semantics explicitly; adversarially review every retained module
  against the new domain.
- **[A large reframe can discard proven mechanics unnecessarily]** → Evaluate each existing seam by
  responsibility and public contract; retain mechanics only when the new requirements exercise
  them honestly.
- **[Strategy configuration can grow into a policy DSL]** → Use a small closed set of presets,
  enums, thresholds, and triggers; require a new concrete use case before adding generality.

## Migration Plan

1. Update `PROJECT.md`, `BACKLOG.md`, naming documentation, and delta specs so the new boundary is
   explicit before product code is rewritten.
2. Introduce dossier/frontmatter types, append-only events, SQLite persistence, and archive
   integrity independently of Agent execution.
3. Narrow the Agent adapter to opaque answers and add respondent, arbitration, and evaluator roles.
4. Add synchronous deliberation, conservative dissent transitions, strategy selection, and durable
   recovery.
5. Replace the CLI with dossier lifecycle and human-decision commands; remove workspace execution
   surfaces and obsolete tests.
6. Run adversarial apply review and the complete Definition of Done before syncing delta specs.

Because version 0.1.0 has not shipped, the SQLite schema and CLI are replaced without a compatibility
migration. Rollback is the branch/PR boundary: no part reaches `main` until the complete change is
verified and squash-merged.

## Open Questions

- Exact CLI nouns and command spelling should be selected during implementation against the naming
  worldview; they must remain deliberative and avoid queue-runtime vocabulary.
- Exact Markdown/frontmatter canonicalization and digest encoding should use the smallest stable
  representation that passes round-trip and tamper tests.
- Whether the first implementation retains all three family dependencies is decided by a static
  composition audit before rewriting the loop, not by roadmap loyalty.
