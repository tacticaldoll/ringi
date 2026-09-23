# Development Flow

This project uses OpenSpec for spec-driven development. `AGENTS.md` is the authoritative
contributor and agent guide; this file is a short checklist.

## One Change

1. Explore current specs and code before editing:
   - `openspec list --specs`
   - `openspec list`
   - read relevant files under `openspec/specs/`
2. Propose the change:
   - `openspec new change "<change>"`
   - write `proposal.md`, `design.md`, `tasks.md`, and delta specs
   - commit as `docs(<change>): propose <summary>`
3. Apply the change:
   - implement against `openspec/changes/<change>/specs/`
   - check off tasks only after the Definition of Done passes
   - commit coherent compiling milestones as `feat(...)` or `fix(...)`
4. Sync verified semantics:
   - promote verified delta specs into `openspec/specs/`, then `git rm -r` the completed change
     directory — its content now lives in `openspec/specs/` and git history; there is no archive
   - commit as `docs(specs): sync <change>`
5. Open a pull request against `main` for the whole change. Branch commits can be as granular as
   you like: the pull request is squash-merged, so its title and body are the durable record.

Every change passes adversarial review at both the propose and apply phases before it is
committed. See `AGENTS.md`'s Commit And Integration Governance for the pull request and
squash-merge rules.

## Commit Granularity

Branch commits should be larger than individual task checkboxes and smaller than an entire risky
feature. Prefer one commit per coherent milestone that builds, tests, and preserves the spec
contract.

Avoid:

- committing unrelated docs, refactors, and behavior together
- checking off `tasks.md` before the Definition of Done passes
- syncing `openspec/specs/` before implementation has been verified

## Definition Of Done

`AGENTS.md` is the single source for the gate list — run its Definition of Done before checking
off tasks or syncing specs. CI runs the same gates on push and pull request.
