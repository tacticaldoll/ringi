//! The convergence seam: ringi's one place that speaks suunta.
//!
//! Ringi owns no convergence mechanism of its own. It projects a revision's residual —
//! every dissent and every risk — onto a suunta `Bearing` of targets and reports a
//! per-target satisfaction verdict; suunta computes the residual and decides whether the
//! dossier has converged. Per `docs/naming.md`'s seam rule, suunta's vocabulary
//! (`Bearing`, `Sigil`, `Sounding`, …) is confined to this module and never names a ringi
//! domain type.
//!
//! Readiness for a human decision is therefore a mechanical fact ([`is_ready`]), not a
//! claim an agent may assert.

use suunta::{
    Bearing, Correction, Fix, Reversibility, Satisfaction, SatisfactionFinding, Sigil, Sounding,
    plan_residual,
};

use crate::revision::{Resolution, Revision};

/// The `Sigil` for a dissent target, stable across soundings.
fn dissent_sigil(id: &uuid::Uuid) -> Sigil {
    Sigil::new(format!("dissent:{id}"))
}

/// The `Sigil` for a risk target, stable across soundings.
fn risk_sigil(id: &uuid::Uuid) -> Sigil {
    Sigil::new(format!("risk:{id}"))
}

/// A provenance-bound resolution is positive certification that a target is met; an open
/// target is unsatisfied. The v1 mapping never emits `Unknown` — it is a forward-compat
/// guarantee suunta honors and that is exercised at the seam by constructing a `Sounding`
/// directly.
fn verdict(resolved_by: &Option<Resolution>) -> Satisfaction {
    match resolved_by {
        // Only a provenance-bound resolution certifies a target satisfied. A resolution
        // without event provenance is an unbacked claim and leaves the target open, so it
        // can never drive convergence.
        Some(res) if !res.provenance.is_empty() => Satisfaction::Satisfied,
        _ => Satisfaction::Unsatisfied,
    }
}

/// Project a revision onto the full deliberation goal (every dissent and risk) and one
/// satisfaction finding per target. The `Bearing` is the whole goal, never pre-filtered to
/// unresolved items; every target carries an explicit finding so a satisfied one is
/// positively certified rather than silently retained.
fn project(revision: &Revision) -> (Bearing<()>, Sounding) {
    let mut targets: Vec<Correction<()>> = Vec::new();
    let mut findings: Vec<SatisfactionFinding> = Vec::new();

    for dissent in &revision.dissents {
        let sigil = dissent_sigil(&dissent.id);
        targets.push(Correction::new(
            sigil.clone(),
            Reversibility::Reversible,
            (),
        ));
        findings.push(SatisfactionFinding {
            target: sigil,
            satisfaction: verdict(&dissent.resolved_by),
        });
    }
    for risk in &revision.risks {
        let sigil = risk_sigil(&risk.id);
        targets.push(Correction::new(
            sigil.clone(),
            Reversibility::Reversible,
            (),
        ));
        findings.push(SatisfactionFinding {
            target: sigil,
            satisfaction: verdict(&risk.resolved_by),
        });
    }

    (
        Bearing::new(targets),
        Sounding::new(Fix::new(findings), vec![]),
    )
}

/// Whether the revision has converged and is ready for a human decision. This is the sole
/// readiness signal; no agent output participates.
#[must_use]
pub fn is_ready(revision: &Revision) -> bool {
    let (bearing, sounding) = project(revision);
    plan_residual(bearing, &sounding).is_converged()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::revision::{Digest, Dissent, EventRef, Resolution, Risk};
    use uuid::Uuid;

    fn revision_with(dissents: Vec<Dissent>, risks: Vec<Risk>) -> Revision {
        Revision {
            revision_id: Uuid::new_v4(),
            parent_digest: None,
            content_digest: Digest("dig".into()),
            original_proposal: "p".into(),
            current_understanding: "u".into(),
            positions: vec![],
            dissents,
            risks,
        }
    }

    fn resolved() -> Option<Resolution> {
        Some(Resolution {
            reason: "handled".into(),
            provenance: vec![EventRef {
                event_id: Uuid::new_v4(),
            }],
        })
    }

    #[test]
    fn empty_residual_is_ready() {
        assert!(is_ready(&revision_with(vec![], vec![])));
    }

    #[test]
    fn an_open_target_is_not_ready() {
        let rev = revision_with(
            vec![Dissent {
                id: Uuid::new_v4(),
                claim: "no".into(),
                resolved_by: None,
            }],
            vec![],
        );
        assert!(!is_ready(&rev));
    }

    #[test]
    fn a_satisfied_target_is_reported_and_converges() {
        // A resolved dissent and a resolved risk must both be positively certified, not
        // omitted; otherwise suunta would retain them and never converge.
        let rev = revision_with(
            vec![Dissent {
                id: Uuid::new_v4(),
                claim: "no".into(),
                resolved_by: resolved(),
            }],
            vec![Risk {
                id: Uuid::new_v4(),
                description: "heat".into(),
                resolved_by: resolved(),
            }],
        );
        assert!(is_ready(&rev));
    }

    #[test]
    fn a_mix_of_resolved_and_open_is_not_ready() {
        let rev = revision_with(
            vec![Dissent {
                id: Uuid::new_v4(),
                claim: "ok".into(),
                resolved_by: resolved(),
            }],
            vec![Risk {
                id: Uuid::new_v4(),
                description: "heat".into(),
                resolved_by: None,
            }],
        );
        assert!(!is_ready(&rev));
    }

    #[test]
    fn a_resolution_without_provenance_does_not_satisfy() {
        // An unbacked resolution (reason but no provenance) must not certify satisfaction,
        // or an agent could converge a dossier with an empty-provenance claim.
        let rev = revision_with(
            vec![Dissent {
                id: Uuid::new_v4(),
                claim: "x".into(),
                resolved_by: Some(Resolution {
                    reason: "trust me".into(),
                    provenance: vec![],
                }),
            }],
            vec![],
        );
        assert!(!is_ready(&rev));
    }

    #[test]
    fn unknown_verdict_blocks_convergence_at_the_seam() {
        // Forward-compat: v1's structural mapping never emits Unknown, but if a target is
        // reported Unknown, suunta must retain it and withhold convergence.
        let sigil = Sigil::new("dissent:probe");
        let bearing = Bearing::new(vec![Correction::new(
            sigil.clone(),
            Reversibility::Reversible,
            (),
        )]);
        let sounding = Sounding::new(
            Fix::new(vec![SatisfactionFinding {
                target: sigil,
                satisfaction: Satisfaction::Unknown,
            }]),
            vec![],
        );
        assert!(!plan_residual(bearing, &sounding).is_converged());
    }
}
