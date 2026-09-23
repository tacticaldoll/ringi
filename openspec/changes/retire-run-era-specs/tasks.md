# Tasks

## 1. Sync

- [ ] 1.1 Add the two `dossier-cli` requirements to `openspec/specs/dossier-cli/spec.md` and verify
  they match the delta
- [ ] 1.2 Delete the eight retired spec directories (`reconcile-loop`, `resumable-runs`,
  `durable-runs`, `cli-run`, `run-assembly`, `builder-execution`, `reviewer-execution`,
  `verification`) and verify `openspec list --specs` no longer lists them
- [ ] 1.3 Delete the three placeholder spec directories (`arbitration-strategy`,
  `dossier-assembly`, `durable-dossiers`) and verify `openspec validate --specs --strict` passes
- [ ] 1.4 Run the Definition of Done and verify it passes
