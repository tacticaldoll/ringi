# motion-authorship Specification

## Purpose

`Motion` is the mechanism by which an arbitration turn changes the dossier's residual: the agent
declares zero or more discrete, provenance-bound `Move`s targeting individual dissents, risks, and
questions, instead of authoring an entire successor `Revision`. Ringi applies the declared moves;
absence of a move for an item is a no-op, never inferred as removing or changing it.

## Requirements

### Requirement: A Move is a discrete, provenance-bound operation on one residual target

An arbitration turn SHALL declare zero or more `Move`s instead of authoring an entire successor
`Revision`. Each `Move` SHALL target exactly one residual item (a dissent, a risk, or a question)
by its stable identifier and SHALL carry whatever provenance that move's kind requires (a
`Resolution` for resolving a dissent, closing a risk, or answering a question; a claim for adding
a dissent; a description for adding a risk; text for asking a question). Ringi applies each
declared `Move` to the current revision to produce the successor; the agent never supplies a
whole successor `Revision`.

#### Scenario: A move resolves a dissent

- **WHEN** an arbitration turn declares a move resolving an open dissent, with a non-empty reason
  and non-empty event provenance
- **THEN** that dissent's resolution is applied to the successor revision

#### Scenario: A move adds a new dissent

- **WHEN** an arbitration turn declares a move adding a dissent with a non-empty claim
- **THEN** a new, unresolved dissent with that claim is appended to the successor revision, with a
  stable id ringi mints

#### Scenario: A move adding a dissent with an empty claim is rejected

- **WHEN** an arbitration turn declares a move adding a dissent with an empty claim
- **THEN** the move is rejected

#### Scenario: A move is rejected without required provenance

- **WHEN** an arbitration turn declares a move resolving a dissent, closing a risk, or answering a
  question with an empty reason or no event provenance
- **THEN** the move is rejected

### Requirement: Absence of a declared move is a no-op, never inferred as removal

A residual item (dissent, risk, or question) with no move targeting it in a given turn SHALL
remain exactly as it was in the parent revision. Silence about an item SHALL NOT be interpreted as
resolving it, removing it, or changing it in any way.

#### Scenario: An untouched dissent survives a turn unchanged

- **WHEN** an arbitration turn declares moves that do not target a given open dissent
- **THEN** that dissent appears in the successor revision exactly as it was in the parent, still
  unresolved

### Requirement: A batch of moves applies atomically

If a turn declares more than one `Move` and any one of them is invalid, none of the moves in that
batch SHALL be applied to the successor revision, and the turn SHALL be rejected as a whole —
matching the all-or-nothing behavior a whole-successor-revision turn has today.

#### Scenario: One invalid move rejects the whole batch

- **WHEN** an arbitration turn declares three moves and one of them is invalid (e.g. missing
  provenance)
- **THEN** none of the three moves are applied, and the turn is rejected

#### Scenario: A batch of entirely valid moves all apply together

- **WHEN** an arbitration turn declares multiple valid moves targeting different residual items
- **THEN** all of them are applied to the same successor revision

### Requirement: Ringi-owned revision fields are never read from a Move batch

`original_proposal`, `revision_id`, `parent_digest`, and `content_digest` SHALL NOT be supplied by
the agent in any form. Ringi carries `original_proposal` forward from the parent revision
unconditionally and computes the other three itself, exactly as it already computes
`revision_id`/`parent_digest`/`content_digest` today regardless of agent input.

#### Scenario: The successor's original proposal always matches the parent's

- **WHEN** any arbitration turn produces a successor revision, regardless of what moves were
  declared
- **THEN** the successor's `original_proposal` is identical to the parent's, because ringi carries
  it forward rather than reading it from the agent's declared moves

### Requirement: The arbitrator prompt shows each residual item's stable id

The arbitrator prompt SHALL show the stable id of every unresolved dissent, unresolved risk, and
open question alongside its text. Ringi — not the agent — mints ids for newly-created risks and
questions, so without this the arbitrator has no way to target an existing item again in a later
turn's `Move`. The prompt SHALL also show the event id of every recent respondent claim alongside
its text, so the arbitrator can cite that claim as event provenance in a `Move` that requires it
(e.g. `AnswerQuestion`, `ResolveDissent`) — without this, the arbitrator can see what was said but
has no identifier through which to reference it as evidence.

#### Scenario: A newly-created risk's id is visible in the next turn's prompt

- **WHEN** an arbitration turn creates a new risk via `AddRisk`, and a later turn's prompt is built
  from the resulting revision
- **THEN** that later prompt shows the risk's ringi-assigned id next to its description

#### Scenario: A respondent claim's event id is visible in the arbitrator prompt

- **WHEN** a respondent's claim is shown to the arbitrator under "Recent Respondent Claims"
- **THEN** the claim's event id is shown alongside its text

### Requirement: A Motion invocation's coordinate stays content-derived

Every `InvocationCoordinate` used to claim a Motion-related invocation (arbitrator or respondent)
SHALL have an `input_digest` computed from the actual revision content that invocation's prompt was
built from, exactly as today. A coordinate's positional fields (`role`, `turn`) MAY narrow which
target an invocation concerns, but MUST NOT replace or approximate the content-derived
`input_digest`.

#### Scenario: A retried invocation against changed content gets a new coordinate

- **WHEN** an invocation is retried after the underlying revision has changed since the prior
  attempt
- **THEN** the retried invocation's coordinate has a different `input_digest`, and is therefore a
  distinct coordinate from the prior attempt's

### Requirement: cadw's vocabulary is confined to the residual-ledger seam

`cadw`'s vocabulary (`TargetId`, `Ledger`, `Move`, `Validator`, `Rejection`) SHALL be imported
only within `crate::residual_ledger`. No `Revision`, `Dissent`, `Risk`, `Question`, or other
ringi domain type SHALL be named using `cadw`'s terms, matching the seam discipline already
enforced for `pacta` (`crate::registry`) and `suunta` (`crate::convergence`).

#### Scenario: cadw is unreachable from outside the residual-ledger seam

- **WHEN** the workspace is checked for module boundaries
- **THEN** `cadw` is imported only from `crate::residual_ledger`, and nowhere else in the crate
