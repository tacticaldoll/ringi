//! `Revision`: the current dossier revision as work SSOT (per `PROJECT.md`), with
//! `Dissent`/`Risk`/`Question` as its addressable, provenance-bound residual items and `Digest`
//! as its content identity.
//!
//! `apply_moves` is the one place a successor revision is produced: an arbitration turn declares
//! zero or more `Move`s, each targeting exactly one residual item, and ringi applies them onto a
//! clone of the current revision — never reading a whole successor object from the agent. A
//! residual item with no move targeting it is carried forward unchanged; there is no way to
//! silently drop it, because removal was never an operation a `Move` batch can express. A
//! resolution/answer must carry a non-empty reason and event provenance — the
//! conservative-retention invariant `BACKLOG.md` states as a settled decision. `apply_moves` also
//! sets the successor's `parent_digest` and recomputes its own content-derived `content_digest`,
//! so the chain of digests is only ever produced here, never assembled by a caller.

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::collections::HashSet;
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

/// A question carried by a revision. Structurally identical to a dissent or risk: a stable id, a
/// description (its text), and an optional provenance-bound answer (reusing `Resolution` — an
/// answer is a resolution in every way that matters to convergence and retention). An unanswered
/// question (no `answered_by`) is a live deliberation target; an answered one is closed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Question {
    pub id: Uuid,
    pub text: String,
    pub answered_by: Option<Resolution>,
}

/// A discrete, provenance-bound operation an arbitration turn declares on exactly one residual
/// target. Ringi applies a batch of these to the current revision to produce the successor; the
/// agent never supplies a whole successor `Revision`. `id` fields reference an existing residual
/// item (a dissent, risk, or question already on the revision); `AddRisk`/`AskQuestion` create a
/// new one instead of targeting an existing id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Move {
    ResolveDissent { id: Uuid, resolution: Resolution },
    AddRisk { description: String },
    CloseRisk { id: Uuid, resolution: Resolution },
    AskQuestion { text: String },
    AnswerQuestion { id: Uuid, resolution: Resolution },
}

impl Move {
    /// The existing residual item this move targets, if any. `AddRisk`/`AskQuestion` create a new
    /// item instead of targeting one, so they have none.
    fn target_id(&self) -> Option<Uuid> {
        match self {
            Move::ResolveDissent { id, .. }
            | Move::CloseRisk { id, .. }
            | Move::AnswerQuestion { id, .. } => Some(*id),
            Move::AddRisk { .. } | Move::AskQuestion { .. } => None,
        }
    }
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
    pub questions: Vec<Question>,
}

impl Revision {
    /// Compute a content digest over the revision's SSOT fields (a SHA-256 hash of their
    /// canonical serialization), so identical content always digests identically and a
    /// change to any SSOT field changes the digest. The digest never depends on
    /// `revision_id` or any other value that varies independently of content.
    pub fn compute_digest(&self) -> Digest {
        let canonical = serde_json::to_vec(&(
            &self.original_proposal,
            &self.current_understanding,
            &self.positions,
            &self.dissents,
            &self.risks,
            &self.questions,
        ))
        .expect("Revision's SSOT fields are always serializable");
        let mut hasher = Sha256::new();
        hasher.update(&canonical);
        Digest(
            hasher
                .finalize()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect(),
        )
    }

    /// Validates one move against `self`'s current state — the target exists, is still open (for
    /// moves that close something), and any resolution carries a non-empty reason and event
    /// provenance. Validation never looks at any other move in the same batch; duplicate
    /// targeting within a batch is caught separately by the caller.
    fn validate_move(&self, mv: &Move) -> Result<(), &'static str> {
        match mv {
            Move::ResolveDissent { id, resolution } => {
                let dissent = self
                    .dissents
                    .iter()
                    .find(|d| d.id == *id)
                    .ok_or("Move targets a dissent that does not exist")?;
                if dissent.resolved_by.is_some() {
                    return Err("Cannot resolve a dissent that is already resolved");
                }
                if resolution.reason.is_empty() {
                    return Err("Dissent resolution requires a reason");
                }
                if resolution.provenance.is_empty() {
                    return Err("Dissent resolution requires event provenance");
                }
            }
            Move::AddRisk { description } => {
                if description.is_empty() {
                    return Err("A new risk requires a non-empty description");
                }
            }
            Move::CloseRisk { id, resolution } => {
                let risk = self
                    .risks
                    .iter()
                    .find(|r| r.id == *id)
                    .ok_or("Move targets a risk that does not exist")?;
                if risk.resolved_by.is_some() {
                    return Err("Cannot close a risk that is already closed");
                }
                if resolution.reason.is_empty() {
                    return Err("Risk resolution requires a reason");
                }
                if resolution.provenance.is_empty() {
                    return Err("Risk resolution requires event provenance");
                }
            }
            Move::AskQuestion { text } => {
                if text.is_empty() {
                    return Err("A new question requires non-empty text");
                }
            }
            Move::AnswerQuestion { id, resolution } => {
                let question = self
                    .questions
                    .iter()
                    .find(|q| q.id == *id)
                    .ok_or("Move targets a question that does not exist")?;
                if question.answered_by.is_some() {
                    return Err("Cannot answer a question that is already answered");
                }
                if resolution.reason.is_empty() {
                    return Err("Question answer requires a reason");
                }
                if resolution.provenance.is_empty() {
                    return Err("Question answer requires event provenance");
                }
            }
        }
        Ok(())
    }

    /// Applies a batch of moves to produce the successor revision. Every move is validated
    /// against `self`'s state before any is applied; if any single move is invalid (including two
    /// moves targeting the same existing item), the whole batch is rejected and no move is
    /// applied — matching a whole-successor turn's all-or-nothing behavior. `original_proposal`
    /// is carried forward unconditionally; there is no `Move` variant through which it could be
    /// supplied or altered. A residual item with no move targeting it is carried forward exactly
    /// as it was — absence is a no-op, never inferred as removal. `current_understanding` is the
    /// one whole-text field the arbitrator still authors freely each turn (outside `Move`'s
    /// vocabulary, per design) — passed in directly rather than derived from any move.
    pub fn apply_moves(
        &self,
        current_understanding: String,
        moves: Vec<Move>,
    ) -> Result<Revision, &'static str> {
        let mut touched: HashSet<Uuid> = HashSet::new();
        for mv in &moves {
            if let Some(id) = mv.target_id()
                && !touched.insert(id)
            {
                return Err("A move batch cannot target the same item twice");
            }
            self.validate_move(mv)?;
        }

        let mut next = self.clone();
        next.current_understanding = current_understanding;
        next.revision_id = Uuid::new_v4();
        for mv in moves {
            match mv {
                Move::ResolveDissent { id, resolution } => {
                    let dissent = next
                        .dissents
                        .iter_mut()
                        .find(|d| d.id == id)
                        .expect("validated above");
                    dissent.resolved_by = Some(resolution);
                }
                Move::AddRisk { description } => {
                    next.risks.push(Risk {
                        id: Uuid::new_v4(),
                        description,
                        resolved_by: None,
                    });
                }
                Move::CloseRisk { id, resolution } => {
                    let risk = next
                        .risks
                        .iter_mut()
                        .find(|r| r.id == id)
                        .expect("validated above");
                    risk.resolved_by = Some(resolution);
                }
                Move::AskQuestion { text } => {
                    next.questions.push(Question {
                        id: Uuid::new_v4(),
                        text,
                        answered_by: None,
                    });
                }
                Move::AnswerQuestion { id, resolution } => {
                    let question = next
                        .questions
                        .iter_mut()
                        .find(|q| q.id == id)
                        .expect("validated above");
                    question.answered_by = Some(resolution);
                }
            }
        }

        next.parent_digest = Some(self.content_digest.clone());
        next.content_digest = next.compute_digest();
        Ok(next)
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
            questions: vec![],
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
            questions: vec![],
        };
        (base, risk_id)
    }

    fn base_with_unanswered_question() -> (Revision, Uuid) {
        let question_id = Uuid::new_v4();
        let base = Revision {
            revision_id: Uuid::new_v4(),
            parent_digest: None,
            content_digest: Digest("initial".into()),
            original_proposal: "Let's build X".into(),
            current_understanding: "Building X".into(),
            positions: vec![],
            dissents: vec![],
            risks: vec![],
            questions: vec![Question {
                id: question_id,
                text: "Which supplier?".into(),
                answered_by: None,
            }],
        };
        (base, question_id)
    }

    fn provenance() -> Vec<EventRef> {
        vec![EventRef {
            event_id: Uuid::new_v4(),
        }]
    }

    #[test]
    fn an_empty_move_batch_carries_every_residual_item_forward_unchanged() {
        let base = create_base_revision();
        let dissent_id = base.dissents[0].id;

        let successor = base
            .apply_moves("unchanged".into(), vec![])
            .expect("empty batch is valid");
        assert_eq!(successor.parent_digest, Some(base.content_digest.clone()));
        assert_eq!(successor.dissents[0].id, dissent_id);
        assert!(successor.dissents[0].resolved_by.is_none());
        // original_proposal is carried forward unconditionally — there is no field through
        // which a move batch could supply or alter it.
        assert_eq!(successor.original_proposal, base.original_proposal);
    }

    #[test]
    fn a_move_targeting_a_nonexistent_dissent_is_rejected() {
        let base = create_base_revision();
        let err = base
            .apply_moves(
                "unchanged".into(),
                vec![Move::ResolveDissent {
                    id: Uuid::new_v4(),
                    resolution: Resolution {
                        reason: "Tested".into(),
                        provenance: provenance(),
                    },
                }],
            )
            .unwrap_err();
        assert_eq!(err, "Move targets a dissent that does not exist");
    }

    #[test]
    fn resolving_a_dissent_without_provenance_is_rejected() {
        let base = create_base_revision();
        let dissent_id = base.dissents[0].id;

        let err = base
            .apply_moves(
                "unchanged".into(),
                vec![Move::ResolveDissent {
                    id: dissent_id,
                    resolution: Resolution {
                        reason: "Tested and it's fast enough".into(),
                        provenance: vec![], // Missing provenance!
                    },
                }],
            )
            .unwrap_err();
        assert_eq!(err, "Dissent resolution requires event provenance");
    }

    #[test]
    fn resolving_a_dissent_with_provenance_succeeds() {
        let base = create_base_revision();
        let dissent_id = base.dissents[0].id;

        let successor = base
            .apply_moves(
                "unchanged".into(),
                vec![Move::ResolveDissent {
                    id: dissent_id,
                    resolution: Resolution {
                        reason: "Tested and it's fast enough".into(),
                        provenance: provenance(),
                    },
                }],
            )
            .expect("valid resolution applies");
        assert!(successor.dissents[0].resolved_by.is_some());
    }

    #[test]
    fn resolving_an_already_resolved_dissent_is_rejected() {
        let base = create_base_revision();
        let dissent_id = base.dissents[0].id;
        let once_resolved = base
            .apply_moves(
                "unchanged".into(),
                vec![Move::ResolveDissent {
                    id: dissent_id,
                    resolution: Resolution {
                        reason: "Tested".into(),
                        provenance: provenance(),
                    },
                }],
            )
            .expect("first resolution applies");

        let err = once_resolved
            .apply_moves(
                "unchanged".into(),
                vec![Move::ResolveDissent {
                    id: dissent_id,
                    resolution: Resolution {
                        reason: "Tested again".into(),
                        provenance: provenance(),
                    },
                }],
            )
            .unwrap_err();
        assert_eq!(err, "Cannot resolve a dissent that is already resolved");
    }

    #[test]
    fn an_unresolved_risk_survives_an_empty_batch_with_its_id() {
        let (base, risk_id) = base_with_unresolved_risk();
        let successor = base
            .apply_moves("unchanged".into(), vec![])
            .expect("empty batch is valid");
        assert_eq!(successor.risks[0].id, risk_id);
        assert!(successor.risks[0].resolved_by.is_none());
    }

    #[test]
    fn closing_a_risk_without_provenance_is_rejected() {
        let (base, risk_id) = base_with_unresolved_risk();
        let err = base
            .apply_moves(
                "unchanged".into(),
                vec![Move::CloseRisk {
                    id: risk_id,
                    resolution: Resolution {
                        reason: "Mitigated with a heatsink".into(),
                        provenance: vec![],
                    },
                }],
            )
            .unwrap_err();
        assert_eq!(err, "Risk resolution requires event provenance");
    }

    #[test]
    fn closing_a_risk_with_provenance_succeeds() {
        let (base, risk_id) = base_with_unresolved_risk();
        let successor = base
            .apply_moves(
                "unchanged".into(),
                vec![Move::CloseRisk {
                    id: risk_id,
                    resolution: Resolution {
                        reason: "Mitigated with a heatsink".into(),
                        provenance: provenance(),
                    },
                }],
            )
            .expect("valid closure applies");
        assert!(successor.risks[0].resolved_by.is_some());
    }

    #[test]
    fn adding_a_risk_with_empty_description_is_rejected() {
        let base = create_base_revision();
        let err = base
            .apply_moves(
                "unchanged".into(),
                vec![Move::AddRisk {
                    description: String::new(),
                }],
            )
            .unwrap_err();
        assert_eq!(err, "A new risk requires a non-empty description");
    }

    #[test]
    fn adding_a_risk_appends_a_new_open_risk() {
        let base = create_base_revision();
        let successor = base
            .apply_moves(
                "unchanged".into(),
                vec![Move::AddRisk {
                    description: "Newly spotted risk".into(),
                }],
            )
            .expect("valid add-risk applies");
        assert_eq!(successor.risks.len(), 1);
        assert_eq!(successor.risks[0].description, "Newly spotted risk");
        assert!(successor.risks[0].resolved_by.is_none());
    }

    #[test]
    fn an_unanswered_question_survives_an_empty_batch_with_its_id() {
        let (base, question_id) = base_with_unanswered_question();
        let successor = base
            .apply_moves("unchanged".into(), vec![])
            .expect("empty batch is valid");
        assert_eq!(successor.questions[0].id, question_id);
        assert!(successor.questions[0].answered_by.is_none());
    }

    #[test]
    fn answering_a_question_without_provenance_is_rejected() {
        let (base, question_id) = base_with_unanswered_question();
        let err = base
            .apply_moves(
                "unchanged".into(),
                vec![Move::AnswerQuestion {
                    id: question_id,
                    resolution: Resolution {
                        reason: "Acme Corp".into(),
                        provenance: vec![],
                    },
                }],
            )
            .unwrap_err();
        assert_eq!(err, "Question answer requires event provenance");
    }

    #[test]
    fn answering_a_question_with_provenance_succeeds() {
        let (base, question_id) = base_with_unanswered_question();
        let successor = base
            .apply_moves(
                "unchanged".into(),
                vec![Move::AnswerQuestion {
                    id: question_id,
                    resolution: Resolution {
                        reason: "Acme Corp".into(),
                        provenance: provenance(),
                    },
                }],
            )
            .expect("valid answer applies");
        assert!(successor.questions[0].answered_by.is_some());
    }

    #[test]
    fn asking_a_question_with_empty_text_is_rejected() {
        let base = create_base_revision();
        let err = base
            .apply_moves(
                "unchanged".into(),
                vec![Move::AskQuestion {
                    text: String::new(),
                }],
            )
            .unwrap_err();
        assert_eq!(err, "A new question requires non-empty text");
    }

    #[test]
    fn asking_a_question_appends_a_new_open_question() {
        let base = create_base_revision();
        let successor = base
            .apply_moves(
                "unchanged".into(),
                vec![Move::AskQuestion {
                    text: "Which supplier?".into(),
                }],
            )
            .expect("valid ask-question applies");
        assert_eq!(successor.questions.len(), 1);
        assert_eq!(successor.questions[0].text, "Which supplier?");
        assert!(successor.questions[0].answered_by.is_none());
    }

    #[test]
    fn a_batch_targeting_the_same_item_twice_is_rejected() {
        let base = create_base_revision();
        let dissent_id = base.dissents[0].id;
        let err = base
            .apply_moves(
                "unchanged".into(),
                vec![
                    Move::ResolveDissent {
                        id: dissent_id,
                        resolution: Resolution {
                            reason: "First".into(),
                            provenance: provenance(),
                        },
                    },
                    Move::ResolveDissent {
                        id: dissent_id,
                        resolution: Resolution {
                            reason: "Second".into(),
                            provenance: provenance(),
                        },
                    },
                ],
            )
            .unwrap_err();
        assert_eq!(err, "A move batch cannot target the same item twice");
    }

    #[test]
    fn a_batch_with_one_invalid_move_applies_none_of_them() {
        // A dissent and a risk in the same revision, so one valid move (close the risk) and one
        // invalid move (resolve the dissent with no provenance) can be declared in one batch.
        let dissent_id = Uuid::new_v4();
        let (mut base, risk_id) = base_with_unresolved_risk();
        base.dissents.push(Dissent {
            id: dissent_id,
            claim: "Also too slow".into(),
            resolved_by: None,
        });

        let err = base
            .clone()
            .apply_moves(
                "unchanged".into(),
                vec![
                    Move::CloseRisk {
                        id: risk_id,
                        resolution: Resolution {
                            reason: "Mitigated".into(),
                            provenance: provenance(),
                        },
                    },
                    Move::ResolveDissent {
                        id: dissent_id,
                        resolution: Resolution {
                            reason: "Missing provenance".into(),
                            provenance: vec![], // invalid
                        },
                    },
                ],
            )
            .unwrap_err();
        assert_eq!(err, "Dissent resolution requires event provenance");

        // The risk's own move was valid, but the batch must still apply nothing: re-attempting
        // the risk closure alone (against the original, untouched base) must still succeed,
        // proving the earlier rejected batch left the risk open.
        let successor = base
            .apply_moves(
                "unchanged".into(),
                vec![Move::CloseRisk {
                    id: risk_id,
                    resolution: Resolution {
                        reason: "Mitigated".into(),
                        provenance: provenance(),
                    },
                }],
            )
            .expect("risk is still open, since the earlier mixed batch applied nothing");
        assert!(successor.risks[0].resolved_by.is_some());
    }

    #[test]
    fn test_identical_content_produces_the_same_digest() {
        let a = create_base_revision();
        let mut b = create_base_revision();
        // Vary only fields that are not part of the SSOT content: identity and the
        // (pre-digest) content_digest placeholder.
        b.revision_id = Uuid::new_v4();
        b.dissents[0].id = a.dissents[0].id;

        assert_eq!(a.compute_digest(), b.compute_digest());
    }

    #[test]
    fn test_changed_content_produces_a_different_digest() {
        let a = create_base_revision();
        let mut b = a.clone();
        b.current_understanding = "Building Y instead".into();

        assert_ne!(a.compute_digest(), b.compute_digest());
    }
}
