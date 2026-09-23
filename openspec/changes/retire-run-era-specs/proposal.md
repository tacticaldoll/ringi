# Proposal

## Why

The dossier pivot deleted the run engine (`reconcile.rs`, `run.rs`, `verify.rs`, `config.rs`,
`ringi run`/`resume`/`status`) but left eight shipped specs describing it, so `openspec/specs/` no
longer matches the code. It also left three requirement-less "(Generated)" placeholder specs
(`arbitration-strategy`, `dossier-assembly`, `durable-dossiers`) that make
`openspec validate --specs --strict` fail.

## What Changes

- Retire `reconcile-loop`, `resumable-runs`, `durable-runs`, `cli-run`, `run-assembly`,
  `builder-execution`, `reviewer-execution`, and `verification`: every requirement is REMOVED with
  a Reason and a Migration, and sync deletes each emptied spec.
- Carry two `durable-runs` requirements the code still satisfies into `dossier-cli`, restated for
  dossiers: `ringi init` provisions `.ringi/state.sqlite` and the dossier schema idempotently, and
  the dossier store and the pacta registry open the same file. The old "user-scope" wording is not
  carried: the path is relative to the working directory, and `init` does not create the registry
  table (it is created when the registry is first opened), so neither is claimed.
- Delete the three empty placeholder specs. Their intended content, the delta specs of
  `openspec/changes/reframe-ringi-deliberation`, was never committed to any ref, so nothing can be
  restored and no requirements are invented.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `dossier-cli`: gains the init-provisioning and shared-store requirements.
- `reconcile-loop`, `resumable-runs`, `durable-runs`, `cli-run`, `run-assembly`,
  `builder-execution`, `reviewer-execution`, `verification`: every requirement removed; the
  capability is retired.

## Impact

Specs only. No product code, manifest, or lockfile changes. The unused `AgentRole::Builder` and
`AgentRole::Reviewer` variants and run-era wording in code comments are left for a code change.
After sync, `openspec validate --specs --strict` passes.
