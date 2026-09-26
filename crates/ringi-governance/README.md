# ringi-governance

Executable architectural governance for the Ringi workspace — the Tianheng constitution.

This crate is an internal gate, not a published library (`publish = false`). It depends only on
the [Tianheng](https://github.com/tacticaldoll/tianheng) composed adopter surface and holds the
seam discipline of `docs/domain-language.md` — pacta's vocabulary confined to `registry`, suunta's
to `convergence`, cadw's to `residual_ledger` — its own dependency independence, workspace
coverage, and the accepted constitution's generated projection, `AGENTS.ringi-law.md`. Ringi does
I/O by design, so the gate carries no sans-I/O teeth.

The seam boundaries observe the library root (`src/lib.rs`) and the binary root (`src/main.rs`):
Tianheng judges an external-crate confinement in every compiled root: in each root a brick may be
imported only from a module at its seam's path, and the binary root declares no such module.

Run it from the workspace root:

```sh
cargo run -p ringi-governance -- check --manifest-path Cargo.toml
```

Regenerate the projection after a deliberate, reviewed law change:

```sh
BLESS=1 cargo test -p ringi-governance law_projection_is_fresh
```

Part of [Ringi](https://github.com/tacticaldoll/ringi).

## License

Licensed under either of [Apache-2.0](https://github.com/tacticaldoll/ringi/blob/main/LICENSE-APACHE) or [MIT](https://github.com/tacticaldoll/ringi/blob/main/LICENSE-MIT), at your option.
