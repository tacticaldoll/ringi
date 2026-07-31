# deliberation-dossier Specification

## Purpose

A dossier's public working state is a revision carrying the residual — the dissents, risks, and
questions still open for deliberation. All three are addressable, provenance-bound, conservatively
retained, and durably persisted, so convergence can be computed mechanically and survives a store
round-trip.

## Requirements

### Requirement: A risk is an addressable target with a closed state

Each risk in a revision SHALL carry a stable identifier and an optional provenance-bound resolution, mirroring a dissent. A risk with a resolution is closed (`Satisfied`); a risk without one is open (`Unsatisfied`). A risk's identifier MUST remain stable while the risk denotes the same concern, so a `Sigil` derived from it is stable across soundings. A risk resolution MUST carry a non-empty reason and non-empty event provenance.

#### Scenario: A risk carries a stable id across a successor revision

- **WHEN** a revision containing an open risk is carried into a successor that leaves the risk open
- **THEN** the risk retains the same identifier in the successor
- **AND** a `Sigil` derived from that identifier is unchanged between the two revisions

#### Scenario: A risk resolution requires reason and provenance

- **WHEN** a successor revision resolves a previously-open risk with an empty reason or no event provenance
- **THEN** the move closing it is rejected

### Requirement: Unresolved risks are conservatively retained

An unresolved risk carried by a revision SHALL NOT be silently dropped by a successor. A successor that omits a previously-unresolved risk MUST be rejected, exactly as for an unresolved dissent.

#### Scenario: Silently dropping an unresolved risk is rejected

- **WHEN** a successor revision omits a risk that was unresolved in its parent
- **THEN** the turn is rejected

### Requirement: Risks are persisted and reloaded

A revision's risks SHALL be persisted with their identifiers and any resolution and provenance, and reconstructed on load, so the residual survives a store round-trip. Commit MUST verify that every event referenced by a risk resolution exists, mirroring dissent provenance verification.

#### Scenario: A persisted risk keeps its id and resolution on reload

- **WHEN** a revision with an open risk and a resolved risk is committed and then reloaded
- **THEN** each reloaded risk has the identifier it was committed with
- **AND** the resolved risk retains its reason and provenance

### Requirement: A question is an addressable target with a closed state

Each question in a revision SHALL carry a stable identifier and an optional provenance-bound
answer, mirroring a dissent and a risk. A question with an answer is closed (`Satisfied`); a
question without one is open (`Unsatisfied`). A question's identifier MUST remain stable while the
question denotes the same open item, so a `Sigil` derived from it is stable across soundings. A
question's answer MUST carry a non-empty reason and non-empty event provenance, reusing the same
`Resolution` shape a dissent or risk resolution uses.

#### Scenario: A question carries a stable id across a successor revision

- **WHEN** a revision containing an open question is carried into a successor that leaves the
  question open
- **THEN** the question retains the same identifier in the successor
- **AND** a `Sigil` derived from that identifier is unchanged between the two revisions

#### Scenario: A question's answer requires reason and provenance

- **WHEN** a successor revision answers a previously-open question with an empty reason or no
  event provenance
- **THEN** the move answering it is rejected

### Requirement: Unanswered questions are conservatively retained

An unanswered question carried by a revision SHALL NOT be silently dropped by a successor. A
successor that omits a previously-unanswered question MUST be rejected, exactly as for an
unresolved dissent or risk.

#### Scenario: Silently dropping an unanswered question is rejected

- **WHEN** a successor revision omits a question that was unanswered in its parent, without an
  `AnswerQuestion` move targeting it
- **THEN** the turn is rejected

### Requirement: Questions are persisted and reloaded

A revision's questions SHALL be persisted with their identifiers and any answer and provenance, and
reconstructed on load, so the residual survives a store round-trip. Commit MUST verify that every
event referenced by a question's answer exists, mirroring dissent and risk provenance
verification.

#### Scenario: A persisted question keeps its id and answer on reload

- **WHEN** a revision with an open question and an answered question is committed and then
  reloaded
- **THEN** each reloaded question has the identifier it was committed with
- **AND** the answered question retains its reason and provenance

### Requirement: The v1 deliberation goal comprises dissents, risks, and questions

A revision's deliberation goal (the suunta `Bearing`) SHALL comprise every dissent, every risk, and
every question as a target, each with a stable `Sigil`. The residual is the subset suunta does not
certify satisfied: a dissent, risk, or question with a provenance-bound resolution/answer is
`Satisfied` and excluded, an open one is retained. Conditions remain dossier-level and are not
targets of this revision-level goal.

#### Scenario: Goal enumerates all dissents, risks, and questions; residual omits satisfied ones

- **WHEN** the deliberation goal for a revision is enumerated
- **THEN** it contains one target per dissent, one target per risk, and one target per question,
  resolved/answered or not
- **AND** after verdicts are applied, the residual omits every target with a provenance-bound
  resolution or answer

### Requirement: A revision's content digest is derived from its content

A revision's `content_digest` SHALL be a value computed from its SSOT content (`original_proposal`,
`current_understanding`, `positions`, `dissents`, `risks`, `questions`) such that two revisions with
identical content in those fields have identical digests, and a revision whose content differs in
any of those fields has a different digest. The digest MUST NOT be derived from the revision's
identifier or any other value that varies independently of content. This computation MUST be
identical for a dossier's initial revision and for every successor produced by applying a `Move`
batch.

#### Scenario: Identical content produces the same digest

- **WHEN** two revisions are constructed with the same `original_proposal`, `current_understanding`,
  `positions`, `dissents`, `risks`, and `questions`
- **THEN** their computed content digests are equal

#### Scenario: Changing content changes the digest

- **WHEN** a revision's `current_understanding` (or any other SSOT field, including `questions`)
  differs from another revision's
- **THEN** their computed content digests are not equal

#### Scenario: The initial revision's digest is content-derived, not a literal

- **WHEN** a dossier is submitted and its initial revision is committed
- **THEN** the initial revision's `content_digest` is computed from its content by the same
  computation applying a `Move` batch uses for a successor, not a fixed literal value

### Requirement: A revision's original proposal is immutable across successors

`original_proposal` SHALL be identical between a revision and every successor produced from it,
enforced structurally: the agent declaring a `Move` batch has no way to supply or alter
`original_proposal` at all, since it is not part of a `Move`'s shape. Ringi carries it forward from
the parent revision unconditionally on every successor.

#### Scenario: A successor's original proposal always equals its parent's

- **WHEN** any successor revision is produced by applying a `Move` batch to a parent revision
- **THEN** the successor's `original_proposal` is identical to the parent's, with no possible input
  path for it to differ
