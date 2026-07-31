# cli-run Specification

## Purpose

How ringi's command surface drives a run: `ringi run` and `ringi init` turn a config file plus
command-line arguments into a `RunConfig`, drive a run through the composition root
(`run_from_config`) over the in-memory reference backend, and present the outcome with an exit
code. It owns command-surface wiring and presentation only — no lifecycle, convergence, or
verification of its own (those stay the composed loop's and the seams'), and, for now, no
durability (the run is in-memory; a durable backend and resume are a later phase).

## Requirements

### Requirement: The Run Command Drives A Run And Presents Its Outcome
The `ringi run` command SHALL take a workspace path and a task on the command line, assemble a
`RunConfig` from those arguments and the loaded config file, drive the run through the composition
root (`run_from_config`) over the in-memory reference backend, and present the run's outcome to
the user. The command SHALL report whether the run converged, how many rounds it took, and any
findings left open. The command's exit status SHALL be success if and only if the run converged;
a run that reaches the round limit without converging SHALL exit non-zero. The command SHALL only
wire and present — it SHALL NOT itself sequence rounds, decide convergence, or verify (those stay
the composed loop's and the seams').

#### Scenario: A converging run reports success
- **WHEN** `ringi run` drives a run that converges within the round limit
- **THEN** it prints the outcome (converged, round count, no open findings) and exits with a success status

#### Scenario: A non-converging run reports failure
- **WHEN** `ringi run` drives a run that reaches the round limit without converging
- **THEN** it prints the outcome including the still-open findings and exits with a non-zero status

#### Scenario: The command only wires and presents
- **WHEN** `ringi run` drives a run
- **THEN** convergence is decided by the composed loop and verification by the Verify seam, and the command contributes no sequencing, completion calculation, or verification of its own

### Requirement: The Config File Supplies Run Parameters
Ringi SHALL read the parameters not given on the command line from a config file: the Agent CLI
program and its arguments, the verification commands (each a program, an argument vector, and a
timeout), and the per-invocation agent timeout. The config file SHALL be TOML. The workspace and
the task SHALL come from the command line, not the config file, so one config serves many runs.
A config file that is missing, unreadable, or malformed SHALL cause the command to fail with a
clear diagnostic and a non-zero exit status, never a partial or default-substituted run.

#### Scenario: A valid config populates the run
- **WHEN** `ringi run` loads a well-formed config file
- **THEN** the Agent CLI, verification commands, and timeout from the file populate the `RunConfig`, combined with the workspace and task from the command line

#### Scenario: A malformed config fails clearly
- **WHEN** the config file is missing, unreadable, or not valid TOML for the expected shape
- **THEN** the command prints a clear diagnostic naming the problem and exits non-zero without starting a run

### Requirement: The Init Command Scaffolds A Config File
The `ringi init` command SHALL write a default, commented config file that a user can edit and
then run. `init` SHALL NOT overwrite an existing config file; if one is already present it SHALL
report that and leave the file unchanged. The scaffolded file SHALL be valid input for `ringi run`
once its placeholder values are filled in.

#### Scenario: Init writes a default config
- **WHEN** `ringi init` runs in a location with no existing config file
- **THEN** it writes a commented default config file and reports where it was written

#### Scenario: Init does not clobber an existing config
- **WHEN** `ringi init` runs where a config file already exists
- **THEN** it leaves the existing file unchanged and reports that it was not overwritten
