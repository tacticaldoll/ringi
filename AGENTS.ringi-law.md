# Ringi Tianheng Law Projection

This file is generated from `constitution()` in `crates/ringi-governance/src/main.rs`.
The Rust declaration is authoritative; do not edit the projection by hand.
Regenerate it with `BLESS=1 cargo test -p ringi-governance law_projection_is_fresh`.

# Constitution: ringi

## Static boundaries

### `ringi::crate::convergence` (module)

> suunta's vocabulary (Bearing, Sigil, Sounding, ...) enters ringi's library root only through the convergence seam: no module outside `crate::convergence` makes a `use` import of suunta. Coverage is partial: the binary root (src/main.rs), a fully qualified inline path, an `extern crate` declaration, a `use` inside a macro body, and a re-export through the seam are invisible to this import scan, so whether a ringi domain type names suunta's vocabulary stays review-governed — see docs/domain-language.md's seam rule

- **rule**: external crate confined to module (external_crate: suunta)
- **kind**: module · **severity**: enforce · **crate**: ringi

### `ringi::crate::registry` (module)

> pacta's vocabulary (Pact, Claim, Retainer, Registry, lifecycle, ...) enters ringi's library root only through the registry seam: no module outside `crate::registry` makes a `use` import of pacta. Coverage is partial: the binary root (src/main.rs), a fully qualified inline path, an `extern crate` declaration, a `use` inside a macro body, and a re-export through the seam are invisible to this import scan, so whether a ringi domain type names pacta's vocabulary stays review-governed — see docs/domain-language.md's seam rule

- **rule**: external crate confined to module (external_crate: pacta)
- **kind**: module · **severity**: enforce · **crate**: ringi

### `ringi::crate::residual_ledger` (module)

> cadw's vocabulary (TargetId, Ledger, Move, Validator, Rejection, ...) enters ringi's library root only through the residual-ledger seam: no module outside `crate::residual_ledger` makes a `use` import of cadw. Coverage is partial: the binary root (src/main.rs), a fully qualified inline path, an `extern crate` declaration, a `use` inside a macro body, and a re-export through the seam are invisible to this import scan, so whether a ringi domain type names cadw's vocabulary stays review-governed — see docs/domain-language.md's seam rule

- **rule**: external crate confined to module (external_crate: cadw)
- **kind**: module · **severity**: enforce · **crate**: ringi

### `ringi-governance` (crate)

> the governance gate must stay independent of the workspace graph it judges: its normal dependencies are Tianheng's composed adopter surface alone, never an individual governance instrument or a workspace crate under judgment.

- **rule**: restrict dependencies to (only: tianheng)
- **kind**: crate · **severity**: enforce
