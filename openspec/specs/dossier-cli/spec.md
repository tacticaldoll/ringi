# dossier-cli Specification

## Purpose

The `ringi` CLI's thin command layer: each subcommand translates between the on-disk draft
markdown file, the durable `DossierStore`, and the domain modules, without owning domain logic of
its own. This capability covers guarantees about what a command displays or accepts, as distinct
from the domain behavior it delegates to.

## Requirements

### Requirement: Inspect's readiness display matches run_deliberation's root-vs-successor rule

`ringi inspect`'s displayed readiness for a dossier's latest revision SHALL be `true` only when
`run_deliberation` would also treat that revision as ready — that is, the revision has a parent
(is a successor, not the undeliberated root) and its residual has converged.

#### Scenario: An undeliberated root dossier reports not ready

- **WHEN** `inspect` is run on a dossier whose latest revision is the initial root revision (no
  parent), before any turn has run
- **THEN** the displayed readiness is `false`, even though the root's empty residual would
  otherwise satisfy `is_ready` alone

#### Scenario: A converged successor reports ready

- **WHEN** `inspect` is run on a dossier whose latest revision is a successor with a converged
  residual
- **THEN** the displayed readiness is `true`
