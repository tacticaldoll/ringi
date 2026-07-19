# rust-openspec-starter

An opinionated starter for Rust projects that use OpenSpec, ADRs, conventional
commits, and AI-agent-friendly governance from day one.

This repository is intentionally small. It provides the process skeleton for a
new project, not product-specific architecture.

## Use

1. Create a new repository from this starter.
2. Replace placeholder project metadata in `PROJECT.md`, `README.md`, and
   `Cargo.toml`.
3. Install or expose the OpenSpec CLI in your shell.
4. Generate local agent shims for your editor or agent:

   ```bash
   openspec init --tools codex
   # or: openspec init --tools claude,cursor,github-copilot
   ```

5. Start the first project-specific change with OpenSpec:

   ```bash
   openspec new change "initial-project-shape"
   ```

   This change should replace placeholders, choose the real crate layout, add
   the first specs, and make the Rust Definition of Done runnable.

## Included

- `AGENTS.md` - repository rules for AI coding agents and humans.
- `PROJECT.md` - project-specific contract, terminology, and priorities.
- `docs/development-flow.md` - short OpenSpec and commit checklist.
- `docs/adr/` - architecture decision record skeleton.
- `openspec/` - empty OpenSpec structure ready for specs and changes.
- A Rust workspace policy anchor in `Cargo.toml`. It intentionally has no
  crates until the first project-specific change chooses the real layout.

Generated agent shims such as `.codex/` and `.claude/` are per-clone local
files and should not be committed.

## License

Licensed under either of [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT), at your option.
