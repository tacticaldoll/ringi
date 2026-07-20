## ADDED Requirements

### Requirement: A risk is an addressable target with a closed state

Each risk in a revision SHALL carry a stable identifier and an optional provenance-bound resolution, mirroring a dissent. A risk with a resolution is closed (`Satisfied`); a risk without one is open (`Unsatisfied`). A risk's identifier MUST remain stable while the risk denotes the same concern, so a `Sigil` derived from it is stable across soundings. A risk resolution MUST carry a non-empty reason and non-empty event provenance.

#### Scenario: A risk carries a stable id across a successor revision

- **WHEN** a revision containing an open risk is carried into a successor that leaves the risk open
- **THEN** the risk retains the same identifier in the successor
- **AND** a `Sigil` derived from that identifier is unchanged between the two revisions

#### Scenario: A risk resolution requires reason and provenance

- **WHEN** a successor revision resolves a previously-open risk with an empty reason or no event provenance
- **THEN** `propose_successor` rejects the successor

### Requirement: Unresolved risks are conservatively retained

An unresolved risk carried by a revision SHALL NOT be silently dropped by a successor. A successor that omits a previously-unresolved risk MUST be rejected, exactly as for an unresolved dissent.

#### Scenario: Silently dropping an unresolved risk is rejected

- **WHEN** a successor revision omits a risk that was unresolved in its parent
- **THEN** `propose_successor` rejects the successor

### Requirement: Risks are persisted and reloaded

A revision's risks SHALL be persisted with their identifiers and any resolution and provenance, and reconstructed on load, so the residual survives a store round-trip. Commit MUST verify that every event referenced by a risk resolution exists, mirroring dissent provenance verification.

#### Scenario: A persisted risk keeps its id and resolution on reload

- **WHEN** a revision with an open risk and a resolved risk is committed and then reloaded
- **THEN** each reloaded risk has the identifier it was committed with
- **AND** the resolved risk retains its reason and provenance

### Requirement: The v1 deliberation goal comprises dissents and risks

A revision's deliberation goal (the suunta `Bearing`) SHALL comprise every dissent and every risk as a target, each with a stable `Sigil`. The residual is the subset suunta does not certify satisfied: a dissent or risk with a provenance-bound resolution is `Satisfied` and excluded, an open one is retained. In v1 the targets are dissents and risks only; open questions and conditions are not yet targets.

#### Scenario: Goal enumerates all dissents and risks; residual omits satisfied ones

- **WHEN** the deliberation goal for a revision is enumerated
- **THEN** it contains one target per dissent and one target per risk, resolved or not
- **AND** after verdicts are applied, the residual omits every target with a provenance-bound resolution
