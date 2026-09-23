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
(is a successor, not the undeliberated root) and its residual has converged. Once a dossier has
reached a terminal state (`Approved`, `ApprovedWithConditions`, `Rejected`, `Cancelled`,
`Invalidated`), a decision has already been rendered and readiness is no longer a live question;
`inspect` SHALL NOT display a readiness value at all for such a dossier.

#### Scenario: An undeliberated root dossier reports not ready

- **WHEN** `inspect` is run on a dossier whose latest revision is the initial root revision (no
  parent), before any turn has run
- **THEN** the displayed readiness is `false`, even though the root's empty residual would
  otherwise satisfy `is_ready` alone

#### Scenario: A converged successor reports ready

- **WHEN** `inspect` is run on a dossier whose latest revision is a successor with a converged
  residual
- **THEN** the displayed readiness is `true`

#### Scenario: A terminal-state dossier displays no readiness value

- **WHEN** `inspect` is run on a dossier whose state is `Approved`, `ApprovedWithConditions`,
  `Rejected`, `Cancelled`, or `Invalidated`
- **THEN** no `Readiness:` line is printed, even if the latest revision's residual is mechanically
  converged

### Requirement: A dossier draft file's frontmatter is cleanly delimited

The frontmatter JSON written to a dossier draft file SHALL be separated from both surrounding
`---` delimiters by a newline, on every command that writes or rewrites the file.

#### Scenario: A freshly drafted dossier's frontmatter is cleanly delimited

- **WHEN** `ringi draft` creates a new dossier file
- **THEN** the frontmatter JSON is preceded and followed by a newline before each `---` delimiter

#### Scenario: A rewritten dossier's frontmatter stays cleanly delimited

- **WHEN** `ringi submit` or a lifecycle transition rewrites a dossier file's frontmatter
- **THEN** the frontmatter JSON is preceded and followed by a newline before each `---` delimiter

### Requirement: Init provisions the durable store without destroying existing data

`ringi init` SHALL provision the durable store — creating the `.ringi/` directory under the
working directory, the `state.sqlite` database, and the dossier schema — so later commands have a
store to record into. Provisioning SHALL be idempotent: running `init` where the store already
exists SHALL leave its schema and recorded dossiers intact.

#### Scenario: Init creates the store

- **WHEN** `ringi init` runs in a directory with no `.ringi/state.sqlite`
- **THEN** it creates the database and the dossier schema

#### Scenario: Init does not destroy existing dossiers

- **WHEN** `ringi init` runs where the store already exists
- **THEN** the existing database and its recorded dossiers are left intact

### Requirement: Dossier state and registry state share one store

The dossier commands SHALL open the dossier store and the pacta registry over the same SQLite
file, so ringi's domain tables and the registry's claim/settle state live in one database rather
than two stores.

#### Scenario: Registry and domain state share one database

- **WHEN** a command that invokes an Agent CLI (such as `ringi continue` or `ringi evaluate`)
  opens both the dossier store and the registry
- **THEN** both are opened over the same `.ringi/state.sqlite` file
