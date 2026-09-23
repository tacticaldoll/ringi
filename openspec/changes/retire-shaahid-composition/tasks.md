# Tasks

## 1. Apply

- [x] 1.1 Delete `sync_specs.py` and verify `git grep sync_specs` finds no caller

## 2. Sync

- [ ] 2.1 Merge the `reconcile-loop`, `resumable-runs`, and `naming-worldview` deltas into
  `openspec/specs/`, placing the renamed resume requirement where the old one stood, and verify
  each edited requirement matches its delta
- [ ] 2.2 Drop the shaahid sentences from the `reconcile-loop` and `resumable-runs` Purposes and
  verify `grep -rni shaahid openspec/specs` finds nothing
- [ ] 2.3 Run `openspec validate --specs --strict` and verify the only failures are the three
  pre-existing placeholder specs
- [ ] 2.4 Run the Definition of Done and verify it passes
