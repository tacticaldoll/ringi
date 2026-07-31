//! Mechanizes the second half of `docs/naming.md`'s seam rule: `naming-guard.sh` checks that a
//! banned generic word never names a declaration; this checks that a brick crate's own imports
//! stay confined to its seam module. Runs as part of `cargo test --workspace`, so a future change
//! that lets `suunta` leak outside `crate::convergence` fails the existing Definition of Done gate
//! instead of relying on review to notice.

use std::path::PathBuf;
use tianheng::prelude::*;

fn constitution() -> Constitution {
    Constitution::new("ringi").boundary(
        ModuleBoundary::in_crate("ringi")
            .module("crate::convergence")
            .confine_external_crate("suunta")
            .because(
                "suunta's vocabulary (Bearing, Sigil, Sounding, ...) is confined to the \
                 convergence seam and never names a ringi domain type — see docs/naming.md's \
                 seam rule",
            ),
    )
}

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")
}

#[test]
fn suunta_is_confined_to_the_convergence_seam() {
    let outcome = check(constitution().static_boundaries(), &manifest());
    assert_eq!(outcome.exit_code(), 0, "{outcome:?}");
}

fn pacta_constitution() -> Constitution {
    Constitution::new("ringi").boundary(
        ModuleBoundary::in_crate("ringi")
            .module("crate::registry")
            .confine_external_crate("pacta")
            .because(
                "pacta's vocabulary (Pact, Claim, Retainer, Registry, lifecycle, ...) is \
                 confined to the registry seam and never names a ringi domain type — see \
                 docs/naming.md's seam rule",
            ),
    )
}

#[test]
fn pacta_is_confined_to_the_registry_seam() {
    let outcome = check(pacta_constitution().static_boundaries(), &manifest());
    assert_eq!(outcome.exit_code(), 0, "{outcome:?}");
}

fn cadw_constitution() -> Constitution {
    Constitution::new("ringi").boundary(
        ModuleBoundary::in_crate("ringi")
            .module("crate::residual_ledger")
            .confine_external_crate("cadw")
            .because(
                "cadw's vocabulary (TargetId, Ledger, Move, Validator, Rejection, ...) is \
                 confined to the residual-ledger seam and never names a ringi domain type — see \
                 docs/naming.md's seam rule",
            ),
    )
}

#[test]
fn cadw_is_confined_to_the_residual_ledger_seam() {
    let outcome = check(cadw_constitution().static_boundaries(), &manifest());
    assert_eq!(outcome.exit_code(), 0, "{outcome:?}");
}
