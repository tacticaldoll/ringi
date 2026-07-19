# AGENTS.md

Meta-guideline for any agent working in this repository. Read this first, then
`PROJECT.md`.

## What ringi is (and how it is governed)

Ringi is an **application** — a family *leaf* that composes pacta / suunta / shaahid, not a
sans-I/O library brick. So it is governed as an app: a Definition of Done, security tests,
and one discipline unique to ringi — **compose, do not reimplement**. There are no sans-I/O
teeth here; ringi does I/O (subprocess, filesystem, SQLite) by design.

## This Project Uses OpenSpec

Source of truth lives in `openspec/`:

- `openspec/specs/` — the living specification of what the system currently is.
- `openspec/changes/` — active change proposals as delta specs.

Per-agent command files (`.codex/`, `.claude/`, …) are per-clone generated and not
committed. Generate your own after cloning: `openspec init --tools <tool>`.

## Workflow

```text
explore -> propose -> apply -> sync
```

1. **Explore**: think and investigate only. No feature code outside a change.
2. **Propose**: create `proposal.md`, `design.md`, `tasks.md`, and delta specs.
3. **Apply**: implement tasks one at a time, checking each off only after verification.
4. **Sync**: merge verified delta specs into `openspec/specs/` (agent-driven), then remove
   the completed change directory. There is **no `openspec/changes/archive/` folder** —
   archive means deletion; git history keeps the deliberation. Do not run `openspec archive`.

## Adversarial review gate

Every change passes an adversarial self-review at **both** the propose and apply phases
before committing. At minimum, check: does it reimplement a brick's job (lifecycle,
convergence, idempotency) instead of composing it? does it keep the model without execution
authority? does it honor the store-is-truth and limits-first invariants? does it leak a
secret or escape the workspace boundary?

## Rules

- Read the relevant `openspec/specs/` and the active change's artifacts before implementing.
- No feature code without an active change that contains tasks.
- Keep changes minimal and scoped. Never bundle unrelated changes.
- Treat `openspec/specs/` as truth; reflect requirement changes there via sync, not silent
  code edits.
- **Compose, do not reimplement.** If a change grows a `Run`/`Step` engine, a completion
  calculation, or an idempotency scheme that duplicates pacta/suunta/shaahid, stop — that is
  the monolith returning.

## Language

Write OpenSpec artifacts, code comments, and commit messages in English; converse with users
in their language.

## Commit And Integration Governance

### Branch Commits

- Use Conventional Commits: `type(scope): summary`.
- Write the subject in English, lowercase imperative mood, at no more than 72 characters.
- Use the body to record motivation, important decisions, constraints, and verification when that context exists.
- Do not append pull request or issue numbers to the subject or body.
- Development branches may contain multiple coherent commits because the pull request is squash-merged.

### Pull Requests

- Branch from `main` and open every change directly against `main`.
- Make the pull request title the intended squash commit subject.
- Give every pull request a non-empty body that explains why the change is needed, what changed, consequential decisions or tradeoffs, and verification.
- Rebase the branch onto the current `main` before final verification.
- Do not introduce a release integration branch between a change and `main`.

### Squash Merges

- Squash-merge every verified pull request into `main`.
- Make the squash commit subject exactly the approved pull request title.
- Give every squash commit a non-empty body distilled from the approved pull request body.
- Do not append a pull request number, issue number, or URL to the squash subject or body.
- Every content-changing commit on `main`, including release preparation, must come from a squash-merged pull request.
- Keep `main` releasable after every merge.

### Attribution

- Do not include AI, agent, model, tool, automation, or generation attribution in commits, pull requests, tags, changelogs, or release notes.
- A `Co-authored-by` trailer is allowed only for a real human contributor.

### Release Finalization

- Prepare release content in a pull request whose squash subject is exactly `chore(release): prepare X.Y.Z`.
- Give the release preparation squash commit a non-empty body describing scope, compatibility, metadata changes, and verification.
- Run the complete Definition of Done after that commit reaches `main`.
- Finalize with annotated tag `vX.Y.Z` on that commit, with message exactly `release: X.Y.Z`.
- Push the tag without another commit. Release branches and empty release commits are not part of the flow.

## Definition Of Done

Run from the workspace root before checking off a task or syncing. This is the single source
for the gate list.

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo deny check
./scripts/naming-guard.sh   # naming-worldview guard — see docs/naming.md
```

As the surface grows, the DoD gains: the security tests (path traversal, symlink escape,
secret redaction, argument injection — see `PROJECT.md` invariants) and running the app end
to end on a fixture. If a command cannot run in the current environment, report that
explicitly.
