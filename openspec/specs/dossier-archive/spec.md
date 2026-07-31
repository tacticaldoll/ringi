# dossier-archive Specification

## Purpose

Rendering a terminal dossier's archive: a human-readable, integrity-bound record of its final SSOT
and every recorded event — public claims and sealed evaluator reasoning alike — per `PROJECT.md`'s
Archive and Sealed-evaluation invariants. A record only: it grants no execution authority and
triggers no workspace effect.

## Requirements

### Requirement: A rendered archive includes every recorded event

`render_archive` SHALL render every event recorded for the dossier, in commit order: every
`Public` event's content under a Public Event Index, and every `Sealed` event's evaluator and
reasoning under a Sealed Audit Section. Neither section SHALL be omitted; an empty section SHALL
render an explicit placeholder rather than disappearing.

#### Scenario: Public events render in commit order

- **WHEN** a dossier has recorded public events across multiple turns
- **THEN** the rendered archive's Public Event Index lists their content in the order they were committed

#### Scenario: Sealed evaluations render for human audit

- **WHEN** a dossier has a sealed evaluator-reasoning event recorded
- **THEN** the rendered archive's Sealed Audit Section includes that evaluator and its reasoning

#### Scenario: A dossier with no events of a kind still renders that section

- **WHEN** a dossier has no sealed events (or no public events)
- **THEN** the corresponding section still renders, with an explicit placeholder noting none exist

### Requirement: The archive's integrity digest covers the rendered event content

The SHA-256 integrity digest appended to a rendered archive SHALL be computed over the full
rendered text, including the now-real Public Event Index and Sealed Audit Section content, so the
digest is not vulnerable to those sections having been tampered with after rendering.

#### Scenario: The digest changes if rendered event content changes

- **WHEN** two archive renderings of the same dossier differ only in their Public Event Index or Sealed Audit Section content
- **THEN** their integrity digests differ

### Requirement: A rendered archive includes every dissent and its final status

`render_archive` SHALL render a `## Dissents` section listing every dissent on the dossier's final
revision, each as a checkbox line showing its claim and whether it was ultimately resolved
(`resolved_by` is `Some`). The section SHALL NOT be omitted for a dossier with no dissents; it
SHALL render an explicit placeholder instead.

#### Scenario: A resolved dissent renders as checked

- **WHEN** a dossier's final revision has a dissent whose `resolved_by` is `Some`
- **THEN** the rendered archive's Dissents section shows that dissent's claim as a checked box

#### Scenario: An unresolved dissent renders as unchecked

- **WHEN** a dossier's final revision has a dissent whose `resolved_by` is `None`
- **THEN** the rendered archive's Dissents section shows that dissent's claim as an unchecked box

#### Scenario: A dossier with no dissents still renders the section

- **WHEN** a dossier's final revision has no dissents
- **THEN** the rendered archive still includes a Dissents section, with an explicit placeholder
  noting none exist

### Requirement: A rendered archive includes every risk and its final status

`render_archive` SHALL render a `## Risks` section listing every risk on the dossier's final
revision, each as a checkbox line showing its description and whether it was ultimately closed
(`resolved_by` is `Some`). The section SHALL NOT be omitted for a dossier with no risks; it SHALL
render an explicit placeholder instead.

#### Scenario: A closed risk renders as checked

- **WHEN** a dossier's final revision has a risk whose `resolved_by` is `Some`
- **THEN** the rendered archive's Risks section shows that risk's description as a checked box

#### Scenario: An open risk renders as unchecked

- **WHEN** a dossier's final revision has a risk whose `resolved_by` is `None`
- **THEN** the rendered archive's Risks section shows that risk's description as an unchecked box

#### Scenario: A dossier with no risks still renders the section

- **WHEN** a dossier's final revision has no risks
- **THEN** the rendered archive still includes a Risks section, with an explicit placeholder
  noting none exist

### Requirement: A rendered archive includes every question and its final status

`render_archive` SHALL render a `## Questions` section listing every question on the dossier's
final revision, each as a checkbox line showing its text and whether it was ultimately answered
(`answered_by` is `Some`). The section SHALL NOT be omitted for a dossier with no questions; it
SHALL render an explicit placeholder instead.

#### Scenario: An answered question renders as checked

- **WHEN** a dossier's final revision has a question whose `answered_by` is `Some`
- **THEN** the rendered archive's Questions section shows that question's text as a checked box

#### Scenario: An open question renders as unchecked

- **WHEN** a dossier's final revision has a question whose `answered_by` is `None`
- **THEN** the rendered archive's Questions section shows that question's text as an unchecked box

#### Scenario: A dossier with no questions still renders the section

- **WHEN** a dossier's final revision has no questions
- **THEN** the rendered archive still includes a Questions section, with an explicit placeholder
  noting none exist

### Requirement: A rendered archive includes every condition and its final status

`render_archive` SHALL render a `## Conditions` section listing every condition on the dossier's
final revision, each as a checkbox line showing its description and whether it was ultimately
satisfied (`resolved_by` is `Some`). The section SHALL NOT be omitted for a dossier with no
conditions; it SHALL render an explicit placeholder instead.

#### Scenario: A satisfied condition renders as checked

- **WHEN** a dossier's final revision has a condition whose `resolved_by` is `Some`
- **THEN** the rendered archive's Conditions section shows that condition's description as a
  checked box

#### Scenario: An unsatisfied condition renders as unchecked

- **WHEN** a dossier's final revision has a condition whose `resolved_by` is `None`
- **THEN** the rendered archive's Conditions section shows that condition's description as an
  unchecked box

#### Scenario: A dossier with no conditions still renders the section

- **WHEN** a dossier's final revision has no conditions
- **THEN** the rendered archive still includes a Conditions section, with an explicit placeholder
  noting none exist
