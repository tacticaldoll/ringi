# run-assembly Specification

## Purpose

How ringi assembles a run from its configuration: the composition root that turns a `RunConfig`
into the wired production seams (Build, Review, Verify) and drives the round loop over a registry
backend to a reported outcome. It owns wiring only — no lifecycle, convergence, or idempotency of
its own (those stay the family's), and no presentation or persistence (those are the caller's).

## Requirements

### Requirement: Ringi Assembles A Run From Its Configuration
Ringi SHALL assemble a run from a `RunConfig` — the workspace, the Agent CLI (program and
arguments), the task, the verification commands, and a per-invocation timeout — by constructing
the production Build, Review, and Verify seams and driving the round loop over a supplied registry
backend, returning the run's report. The run's round bound is the round loop's own mandatory
limit (a per-run configurable bound is a later refinement). Assembly SHALL be backend-agnostic:
the registry backend SHALL be supplied to the assembly, so the same assembly runs over the
in-memory reference backend and over a durable backend. The single configured Agent CLI SHALL
back both the Build and Review roles, which are distinguished by their role and prompt, not by
separate programs. Assembly SHALL return the report and SHALL NOT itself present or persist it.

#### Scenario: A configured run is assembled and driven to convergence
- **WHEN** ringi assembles a run from a `RunConfig` and drives it over a registry backend
- **THEN** it constructs the Builder, Reviewer, and Verification from the config, runs the round loop, and returns the run's report

#### Scenario: Assembly is backend-agnostic
- **WHEN** the same `RunConfig` is assembled over the in-memory backend and over a durable backend
- **THEN** the assembly is identical and only the supplied registry backend differs
