## ADDED Requirements

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
