use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Digest(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventRef {
    pub event_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resolution {
    pub reason: String,
    pub provenance: Vec<EventRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dissent {
    pub id: Uuid,
    pub claim: String,
    pub resolved_by: Option<Resolution>,
}

/// A risk carried by a revision. Mirrors a dissent: a stable id, a description, and an
/// optional provenance-bound resolution. An unresolved risk (no `resolved_by`) is a live
/// deliberation target; a resolved one is closed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Risk {
    pub id: Uuid,
    pub description: String,
    pub resolved_by: Option<Resolution>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Revision {
    pub revision_id: Uuid,
    pub parent_digest: Option<Digest>,
    pub content_digest: Digest,

    // The SSOT body parts
    pub original_proposal: String,
    pub current_understanding: String,
    pub positions: Vec<String>,
    pub dissents: Vec<Dissent>,
    pub risks: Vec<Risk>,
}

impl Revision {
    /// Compute a pseudo-digest for the revision content.
    /// In a production scenario, this would use a robust cryptographic hash over canonicalized content.
    pub fn compute_digest(&self) -> Digest {
        Digest(format!("digest-{}", self.revision_id))
    }

    /// Creates a successor revision proposal and enforces conservative dissent retention.
    /// Unresolved dissents must either be carried forward as unresolved, or resolved with provenance.
    pub fn propose_successor(&self, mut new_revision: Revision) -> Result<Revision, &'static str> {
        // Enforce structural rejection for unsupported removal of unresolved dissents
        for old_dissent in &self.dissents {
            if old_dissent.resolved_by.is_none() {
                let matching = new_revision
                    .dissents
                    .iter()
                    .find(|d| d.id == old_dissent.id);
                match matching {
                    Some(new_dissent) => {
                        // If it's resolved now, it MUST have provenance.
                        if let Some(res) = &new_dissent.resolved_by {
                            if res.reason.is_empty() {
                                return Err("Dissent resolution requires a reason");
                            }
                            if res.provenance.is_empty() {
                                return Err("Dissent resolution requires event provenance");
                            }
                        }
                    }
                    None => {
                        // Silently removed an unresolved dissent
                        return Err("Cannot silently remove an unresolved dissent");
                    }
                }
            }
        }

        // Enforce conservative retention for risks, mirroring dissents: an unresolved risk
        // must be carried forward, and a newly-resolved one needs a reason and provenance.
        for old_risk in &self.risks {
            if old_risk.resolved_by.is_none() {
                let matching = new_revision.risks.iter().find(|r| r.id == old_risk.id);
                match matching {
                    Some(new_risk) => {
                        if let Some(res) = &new_risk.resolved_by {
                            if res.reason.is_empty() {
                                return Err("Risk resolution requires a reason");
                            }
                            if res.provenance.is_empty() {
                                return Err("Risk resolution requires event provenance");
                            }
                        }
                    }
                    None => {
                        return Err("Cannot silently remove an unresolved risk");
                    }
                }
            }
        }

        new_revision.parent_digest = Some(self.content_digest.clone());
        new_revision.content_digest = new_revision.compute_digest();
        Ok(new_revision)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_base_revision() -> Revision {
        Revision {
            revision_id: Uuid::new_v4(),
            parent_digest: None,
            content_digest: Digest("initial".into()),
            original_proposal: "Let's build X".into(),
            current_understanding: "Building X".into(),
            positions: vec![],
            dissents: vec![Dissent {
                id: Uuid::new_v4(),
                claim: "X is too slow".into(),
                resolved_by: None,
            }],
            risks: vec![],
        }
    }

    fn base_with_unresolved_risk() -> (Revision, Uuid) {
        let risk_id = Uuid::new_v4();
        let base = Revision {
            revision_id: Uuid::new_v4(),
            parent_digest: None,
            content_digest: Digest("initial".into()),
            original_proposal: "Let's build X".into(),
            current_understanding: "Building X".into(),
            positions: vec![],
            dissents: vec![],
            risks: vec![Risk {
                id: risk_id,
                description: "X may overheat".into(),
                resolved_by: None,
            }],
        };
        (base, risk_id)
    }

    #[test]
    fn test_valid_successor_carries_unresolved_dissent() {
        let base = create_base_revision();
        let mut next = base.clone();
        next.revision_id = Uuid::new_v4();

        let successor = base.propose_successor(next);
        assert!(successor.is_ok());
        let succ = successor.unwrap();
        assert_eq!(succ.parent_digest, Some(base.content_digest));
    }

    #[test]
    fn test_silent_removal_of_unresolved_dissent_is_rejected() {
        let base = create_base_revision();
        let mut next = base.clone();
        next.revision_id = Uuid::new_v4();
        next.dissents.clear(); // Silently remove the dissent!

        let err = base.propose_successor(next).unwrap_err();
        assert_eq!(err, "Cannot silently remove an unresolved dissent");
    }

    #[test]
    fn test_resolution_without_provenance_is_rejected() {
        let base = create_base_revision();
        let mut next = base.clone();
        next.revision_id = Uuid::new_v4();

        // Try to resolve without provenance
        next.dissents[0].resolved_by = Some(Resolution {
            reason: "Tested and it's fast enough".into(),
            provenance: vec![], // Missing provenance!
        });

        let err = base.propose_successor(next).unwrap_err();
        assert_eq!(err, "Dissent resolution requires event provenance");
    }

    #[test]
    fn test_valid_resolution_with_provenance() {
        let base = create_base_revision();
        let mut next = base.clone();
        next.revision_id = Uuid::new_v4();

        next.dissents[0].resolved_by = Some(Resolution {
            reason: "Tested and it's fast enough".into(),
            provenance: vec![EventRef {
                event_id: Uuid::new_v4(),
            }],
        });

        let successor = base.propose_successor(next);
        assert!(successor.is_ok());
    }

    #[test]
    fn test_unresolved_risk_carried_forward_keeps_id() {
        let (base, risk_id) = base_with_unresolved_risk();
        let mut next = base.clone();
        next.revision_id = Uuid::new_v4();

        let succ = base.propose_successor(next).expect("carry risk forward");
        assert_eq!(succ.risks[0].id, risk_id);
    }

    #[test]
    fn test_silent_removal_of_unresolved_risk_is_rejected() {
        let (base, _) = base_with_unresolved_risk();
        let mut next = base.clone();
        next.revision_id = Uuid::new_v4();
        next.risks.clear();

        let err = base.propose_successor(next).unwrap_err();
        assert_eq!(err, "Cannot silently remove an unresolved risk");
    }

    #[test]
    fn test_risk_resolution_without_provenance_is_rejected() {
        let (base, _) = base_with_unresolved_risk();
        let mut next = base.clone();
        next.revision_id = Uuid::new_v4();
        next.risks[0].resolved_by = Some(Resolution {
            reason: "Mitigated with a heatsink".into(),
            provenance: vec![],
        });

        let err = base.propose_successor(next).unwrap_err();
        assert_eq!(err, "Risk resolution requires event provenance");
    }

    #[test]
    fn test_valid_risk_resolution_with_provenance() {
        let (base, _) = base_with_unresolved_risk();
        let mut next = base.clone();
        next.revision_id = Uuid::new_v4();
        next.risks[0].resolved_by = Some(Resolution {
            reason: "Mitigated with a heatsink".into(),
            provenance: vec![EventRef {
                event_id: Uuid::new_v4(),
            }],
        });

        assert!(base.propose_successor(next).is_ok());
    }
}
