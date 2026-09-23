# Ringi Tianheng Law Projection

This file is generated from `constitution()` in `crates/ringi-governance/src/main.rs`.
The Rust declaration is authoritative; do not edit the projection by hand.
Regenerate it with `BLESS=1 cargo test -p ringi-governance law_projection_is_fresh`.

# Constitution: ringi

## Static boundaries

### `ringi::crate::convergence` (module)

> suunta's vocabulary (Bearing, Sigil, Sounding, ...) is confined to the convergence seam and never names a ringi domain type — see docs/domain-language.md's seam rule

- **rule**: external crate confined to module (external_crate: suunta)
- **kind**: module · **severity**: enforce · **crate**: ringi

### `ringi::crate::registry` (module)

> pacta's vocabulary (Pact, Claim, Retainer, Registry, lifecycle, ...) is confined to the registry seam and never names a ringi domain type — see docs/domain-language.md's seam rule

- **rule**: external crate confined to module (external_crate: pacta)
- **kind**: module · **severity**: enforce · **crate**: ringi

### `ringi::crate::residual_ledger` (module)

> cadw's vocabulary (TargetId, Ledger, Move, Validator, Rejection, ...) is confined to the residual-ledger seam and never names a ringi domain type — see docs/domain-language.md's seam rule

- **rule**: external crate confined to module (external_crate: cadw)
- **kind**: module · **severity**: enforce · **crate**: ringi

### `ringi-governance` (crate)

> the governance gate must stay independent of the workspace graph it judges: its normal dependencies are Tianheng's composed adopter surface alone, never an individual governance instrument or a workspace crate under judgment.

- **rule**: restrict dependencies to (only: tianheng)
- **kind**: crate · **severity**: enforce
