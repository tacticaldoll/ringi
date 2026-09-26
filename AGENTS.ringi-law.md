# Ringi Tianheng Law Projection

This file is generated from `constitution()` in `crates/ringi-governance/src/main.rs`.
The Rust declaration is authoritative; do not edit the projection by hand.
Regenerate it with `BLESS=1 cargo test -p ringi-governance law_projection_is_fresh`.

# Constitution: ringi

## Static boundaries

### `ringi::crate::convergence` (module)

> suunta's vocabulary (Bearing, Sigil, Sounding, ...) enters ringi's library and binary roots only through the convergence seam: in each of those roots, no module outside `crate::convergence` makes a `use` import of suunta. Coverage is partial: a module the binary root itself declares at the seam's path is permitted like the library's seam, and a fully qualified inline path, an `extern crate` declaration, a `use` inside a macro body, and a re-export through the seam are invisible to this import scan, so whether a ringi domain type names suunta's vocabulary stays review-governed — see docs/domain-language.md's seam rule

- **rule**: external crate confined to module (external_crate: suunta)
- **kind**: module · **severity**: enforce · **crate**: ringi

### `ringi::crate::registry` (module)

> pacta's vocabulary (Pact, Claim, Retainer, Registry, lifecycle, ...) enters ringi's library and binary roots only through the registry seam: in each of those roots, no module outside `crate::registry` makes a `use` import of pacta. Coverage is partial: a module the binary root itself declares at the seam's path is permitted like the library's seam, and a fully qualified inline path, an `extern crate` declaration, a `use` inside a macro body, and a re-export through the seam are invisible to this import scan, so whether a ringi domain type names pacta's vocabulary stays review-governed — see docs/domain-language.md's seam rule

- **rule**: external crate confined to module (external_crate: pacta)
- **kind**: module · **severity**: enforce · **crate**: ringi

### `ringi::crate::residual_ledger` (module)

> cadw's vocabulary (TargetId, Ledger, Move, Validator, Rejection, ...) enters ringi's library and binary roots only through the residual-ledger seam: in each of those roots, no module outside `crate::residual_ledger` makes a `use` import of cadw. Coverage is partial: a module the binary root itself declares at the seam's path is permitted like the library's seam, and a fully qualified inline path, an `extern crate` declaration, a `use` inside a macro body, and a re-export through the seam are invisible to this import scan, so whether a ringi domain type names cadw's vocabulary stays review-governed — see docs/domain-language.md's seam rule

- **rule**: external crate confined to module (external_crate: cadw)
- **kind**: module · **severity**: enforce · **crate**: ringi

### `ringi::crate::agent` (module)

> the agent seam composes the shared subprocess primitive in `crate::exec` rather than hand-rolling a spawn path: in the library root, no module in `crate::agent`'s subtree, its test modules included, calls a function under `std::process::Command` (such as `Command::new`), whether written fully qualified or through a `use` import or alias. A `type` alias or `pub use` of that path inside the subtree also reacts, fail-closed, wherever a glob import can reach it, such as a tests module's `use super::*`. Coverage is partial: otherwise a mention as a type or value does not react; a method called on a `Command` value, a spawn through any other API, and a call a macro constructs from fragments are not observed; modules outside `crate::agent`, and the binary root, which declares no agent module, are not judged by this boundary; so what `crate::exec` itself guarantees (program and arguments, never a shell; a minimized environment; a timeout) stays test- and review-governed

- **rule**: inline symbol path confined to module (confined_prefix: std::process::Command)
- **kind**: module · **severity**: enforce · **crate**: ringi

### `ringi-governance` (crate)

> the governance gate must stay independent of the workspace graph it judges: its normal dependencies are Tianheng's composed adopter surface alone, never an individual governance instrument or a workspace crate under judgment.

- **rule**: restrict dependencies to (only: tianheng)
- **kind**: crate · **severity**: enforce
