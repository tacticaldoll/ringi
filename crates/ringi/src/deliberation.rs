//! Pure prompt-building and arbitration-output application — no I/O, no persistence, no agent
//! invocation. `build_respondent_prompt`/`build_arbitrator_prompt`/
//! `build_condition_evaluator_prompt` each read only a `Revision` and, for the last, one
//! `Condition` in isolation; none reads the raw event log (see `event.rs`'s
//! `RespondentContextProjection` note on why that currently suffices for the sealed-evaluation
//! invariant). `apply_arbitration` is the seam between an agent's raw structured output and the
//! validated successor: it delegates the actual acceptance decision to `Revision::apply_moves`
//! rather than re-deciding it here.

use crate::dossier::Condition;
use crate::revision::{Move, Revision};

/// Build the prompt for a respondent agent.
/// It contains the original proposal, current public revision state (understanding, positions),
/// unresolved items (dissents, risks, questions), and the specific question to answer.
pub fn build_respondent_prompt(question: &str, revision: &Revision) -> String {
    let mut prompt = String::new();
    prompt.push_str("You are a respondent in a deliberation process.\n\n");
    prompt.push_str("## Original Proposal\n");
    prompt.push_str(&revision.original_proposal);
    prompt.push_str("\n\n## Current Understanding\n");
    prompt.push_str(&revision.current_understanding);
    prompt.push('\n');

    if !revision.positions.is_empty() {
        prompt.push_str("\n## Positions\n");
        for pos in &revision.positions {
            prompt.push_str(&format!("- {}\n", pos));
        }
    }

    let unresolved_dissents: Vec<_> = revision
        .dissents
        .iter()
        .filter(|d| d.resolved_by.is_none())
        .collect();
    if !unresolved_dissents.is_empty() {
        prompt.push_str("\n## Unresolved Dissents\n");
        for d in unresolved_dissents {
            prompt.push_str(&format!("- {}\n", d.claim));
        }
    }

    let unresolved_risks: Vec<_> = revision
        .risks
        .iter()
        .filter(|r| r.resolved_by.is_none())
        .collect();
    if !unresolved_risks.is_empty() {
        prompt.push_str("\n## Unresolved Risks\n");
        for r in unresolved_risks {
            prompt.push_str(&format!("- {}\n", r.description));
        }
    }

    let open_questions: Vec<_> = revision
        .questions
        .iter()
        .filter(|q| q.answered_by.is_none())
        .collect();
    if !open_questions.is_empty() {
        prompt.push_str("\n## Open Questions\n");
        for q in open_questions {
            prompt.push_str(&format!("- {}\n", q.text));
        }
    }

    prompt.push_str("\n## Question for you\n");
    prompt.push_str(question);
    prompt.push_str("\n\nPlease provide your answer.");

    prompt
}

/// The structured output expected from an arbitration turn: the freely-authored narrative
/// summary plus a batch of discrete moves on the residual — never a whole successor revision
/// (see `revision.rs`'s `Move`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ArbitrationOutput {
    pub current_understanding: String,
    pub moves: Vec<Move>,
}

/// Applies an arbitration output to a base revision, enforcing structural validity.
/// Returns the new successor revision. Readiness is NOT an output here: it is computed
/// mechanically from the residual by the `convergence` seam.
pub fn apply_arbitration(
    base: &Revision,
    output: ArbitrationOutput,
) -> Result<Revision, &'static str> {
    base.apply_moves(output.current_understanding, output.moves)
}

/// Build the prompt for an arbitrator agent.
/// It contains the full history (simplified as the current revision for now), unresolved items
/// with their stable ids (a `Move` targets an existing item by id, and ringi — not the agent —
/// mints ids for newly-created risks/questions, so the arbitrator has no way to target an item
/// again in a later turn unless its id is shown here), and recent respondent claims (passed as
/// events).
pub fn build_arbitrator_prompt(revision: &Revision, recent_claims: &[String]) -> String {
    let mut prompt = String::new();
    prompt.push_str("You are the arbitrator.\n\n");
    prompt.push_str("## Current SSOT\n");
    prompt.push_str(&revision.current_understanding);
    prompt.push('\n');

    let unresolved_dissents: Vec<_> = revision
        .dissents
        .iter()
        .filter(|d| d.resolved_by.is_none())
        .collect();
    if !unresolved_dissents.is_empty() {
        prompt.push_str("\n## Unresolved Dissents\n");
        for d in unresolved_dissents {
            prompt.push_str(&format!("- [{}] {}\n", d.id, d.claim));
        }
    }

    let unresolved_risks: Vec<_> = revision
        .risks
        .iter()
        .filter(|r| r.resolved_by.is_none())
        .collect();
    if !unresolved_risks.is_empty() {
        prompt.push_str("\n## Unresolved Risks\n");
        for r in unresolved_risks {
            prompt.push_str(&format!("- [{}] {}\n", r.id, r.description));
        }
    }

    let open_questions: Vec<_> = revision
        .questions
        .iter()
        .filter(|q| q.answered_by.is_none())
        .collect();
    if !open_questions.is_empty() {
        prompt.push_str("\n## Open Questions\n");
        for q in open_questions {
            prompt.push_str(&format!("- [{}] {}\n", q.id, q.text));
        }
    }

    if !recent_claims.is_empty() {
        prompt.push_str("\n## Recent Respondent Claims\n");
        for claim in recent_claims {
            prompt.push_str(&format!("- {}\n", claim));
        }
    }

    prompt.push_str(
        "\nPlease provide an updated narrative understanding, and declare zero or more moves on \
         the residual: resolve a dissent, add or close a risk, ask a question, or answer a \
         question. Do not restate items you are not acting on — silence leaves them exactly as \
         they are.",
    );
    // The transport (`agent::parse_metadata`) scans stdout lines in reverse for a single line
    // that parses as JSON, so the arbitrator must emit its structured output as exactly one line
    // of compact JSON — a permanent transport constraint, independent of what that JSON contains.
    prompt.push_str(
        "\n\nEnd your reply with exactly one line of compact JSON (no surrounding prose, \
         no pretty-printing) of the form: {\"current_understanding\": \"...\", \"moves\": \
         [{\"kind\": \"ResolveDissent\", \"id\": \"...\", \"resolution\": {\"reason\": \"...\", \
         \"provenance\": [{\"event_id\": \"...\"}]}}, ...]} — each move's \"kind\" is one of \
         ResolveDissent, AddRisk, CloseRisk, AskQuestion, AnswerQuestion.",
    );
    prompt
}

/// The deterministic trace of why an arbitration session was selected.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SessionProvenance {
    pub strategy: crate::dossier::StrategyPreset,
    pub reason: String,
}

/// Represents the session instruction for arbitration.
pub struct ArbitrationSession {
    pub session_id: String,
    pub provenance: SessionProvenance,
}

impl ArbitrationSession {
    /// Determines the session ID to use for arbitration.
    /// If an escalation trigger is met (like pre-decision review in Balanced), it forces a fresh session.
    pub fn determine(
        settings: &crate::dossier::ArbitrationSettings,
        in_memory_session: Option<String>,
        escalation_triggered: bool,
    ) -> Self {
        use crate::dossier::SessionScope;

        let new_session = |reason: &str| Self {
            session_id: format!("session-{}", uuid::Uuid::new_v4()),
            provenance: SessionProvenance {
                strategy: settings.preset,
                reason: reason.to_string(),
            },
        };

        if escalation_triggered {
            return new_session("escalation trigger matched");
        }

        match settings.session_scope {
            SessionScope::Persistent => {
                if let Some(id) = in_memory_session {
                    Self {
                        session_id: id,
                        provenance: SessionProvenance {
                            strategy: settings.preset,
                            reason: "persistent session reused".to_string(),
                        },
                    }
                } else {
                    new_session("process loss reconstruction")
                }
            }
            SessionScope::FreshPerRound => new_session("fresh per round scope"),
            SessionScope::FreshPerResolution => new_session("fresh per resolution scope"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ConditionVerdict {
    True,
    False,
    Unknown,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConditionEvaluationOutput {
    pub verdict: ConditionVerdict,
    pub reason: String,
    pub provenance: Vec<crate::revision::EventRef>,
}

/// Build the prompt for a condition-evaluator agent, judging exactly one condition.
///
/// Isolated by construction: it contains only the dossier's public SSOT and the single
/// condition under judgment — never another condition, never a prior evaluator's sealed
/// reasoning, never a dissent or risk. Evaluators verify; they do not coach each other.
pub fn build_condition_evaluator_prompt(condition: &Condition, revision: &Revision) -> String {
    let mut prompt = String::new();
    prompt.push_str("You are an isolated condition evaluator.\n\n");
    prompt.push_str("## Original Proposal\n");
    prompt.push_str(&revision.original_proposal);
    prompt.push_str("\n\n## Current Understanding\n");
    prompt.push_str(&revision.current_understanding);
    prompt.push_str("\n\n## Condition to Judge\n");
    prompt.push_str(&condition.description);
    prompt.push_str(
        "\n\nDecide whether this condition is currently satisfied. Respond with your reasoning, \
         then end your reply with exactly one line of compact JSON (no surrounding prose, no \
         pretty-printing) of the form: \
         {\"verdict\": \"True\" | \"False\" | \"Unknown\", \"reason\": \"...\", \"provenance\": []}",
    );
    prompt
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Event, EventPayload, EventVisibility};
    use crate::revision::{Digest, Dissent, EventRef, Resolution};
    use uuid::Uuid;

    #[test]
    fn condition_evaluator_prompt_does_not_leak_another_condition() {
        let revision = Revision {
            revision_id: Uuid::new_v4(),
            parent_digest: None,
            content_digest: Digest("dig".into()),
            original_proposal: "Plan".into(),
            current_understanding: "Plan".into(),
            positions: vec![],
            dissents: vec![],
            risks: vec![],
            questions: vec![],
        };
        let first = Condition {
            id: Uuid::new_v4(),
            description: "Budget is under $1000".into(),
            is_met: false,
        };
        let second = Condition {
            id: Uuid::new_v4(),
            description: "Security review completed".into(),
            is_met: false,
        };

        let prompt = build_condition_evaluator_prompt(&first, &revision);
        assert!(prompt.contains(&first.description));
        assert!(!prompt.contains(&second.description));
    }

    #[test]
    fn arbitrator_prompt_shows_stable_ids_for_unresolved_items() {
        // A Move targets an existing item by id, and ringi (not the agent) mints ids for newly
        // created risks/questions — so the arbitrator prompt must show each item's id, or the
        // arbitrator would have no way to target it again in a later turn.
        let dissent_id = Uuid::new_v4();
        let risk_id = Uuid::new_v4();
        let question_id = Uuid::new_v4();
        let revision = Revision {
            revision_id: Uuid::new_v4(),
            parent_digest: None,
            content_digest: Digest("dig".into()),
            original_proposal: "Plan".into(),
            current_understanding: "Plan".into(),
            positions: vec![],
            dissents: vec![Dissent {
                id: dissent_id,
                claim: "Too slow".into(),
                resolved_by: None,
            }],
            risks: vec![crate::revision::Risk {
                id: risk_id,
                description: "Overheating".into(),
                resolved_by: None,
            }],
            questions: vec![crate::revision::Question {
                id: question_id,
                text: "Which supplier?".into(),
                answered_by: None,
            }],
        };

        let prompt = build_arbitrator_prompt(&revision, &[]);
        assert!(prompt.contains(&dissent_id.to_string()));
        assert!(prompt.contains(&risk_id.to_string()));
        assert!(prompt.contains(&question_id.to_string()));
    }

    #[test]
    fn respondent_prompt_includes_only_unresolved_items() {
        let revision = Revision {
            revision_id: Uuid::new_v4(),
            parent_digest: None,
            content_digest: Digest("dig".into()),
            original_proposal: "Let's build a spaceship".into(),
            current_understanding: "It should go to Mars".into(),
            positions: vec!["Alice: Mars is cool".into()],
            dissents: vec![
                Dissent {
                    id: Uuid::new_v4(),
                    claim: "Too expensive".into(),
                    resolved_by: None,
                },
                Dissent {
                    id: Uuid::new_v4(),
                    claim: "Too far".into(),
                    resolved_by: Some(crate::revision::Resolution {
                        reason: "Warp drive".into(),
                        provenance: vec![],
                    }),
                },
            ],
            risks: vec![
                crate::revision::Risk {
                    id: Uuid::new_v4(),
                    description: "Aliens".into(),
                    resolved_by: None,
                },
                crate::revision::Risk {
                    id: Uuid::new_v4(),
                    description: "Solar flare".into(),
                    resolved_by: Some(crate::revision::Resolution {
                        reason: "Shielding".into(),
                        provenance: vec![],
                    }),
                },
            ],
            questions: vec![
                crate::revision::Question {
                    id: Uuid::new_v4(),
                    text: "Which engine supplier?".into(),
                    answered_by: None,
                },
                crate::revision::Question {
                    id: Uuid::new_v4(),
                    text: "Launch site?".into(),
                    answered_by: Some(crate::revision::Resolution {
                        reason: "Cape Canaveral".into(),
                        provenance: vec![],
                    }),
                },
            ],
        };

        let prompt = build_respondent_prompt("What fuel to use?", &revision);
        assert!(prompt.contains("Let's build a spaceship"));
        assert!(prompt.contains("Mars is cool"));
        assert!(prompt.contains("Too expensive"));
        assert!(!prompt.contains("Too far")); // resolved should not be included
        assert!(prompt.contains("Aliens"));
        assert!(!prompt.contains("Solar flare")); // resolved risk should not be included
        assert!(prompt.contains("Which engine supplier?"));
        assert!(!prompt.contains("Launch site?")); // answered question should not be included
        assert!(prompt.contains("What fuel to use?"));
    }

    #[test]
    fn respondent_answer_is_a_claim_that_does_not_mutate_ssot() {
        // A respondent answers the question
        let answer = "Use liquid hydrogen.";

        // Ringi records it as an event (a claim)
        let event = Event::new_public(EventPayload::PublicRecord(answer.into()), 12345);

        // This event is just data. It cannot mutate a Revision directly, proving it's only a claim.
        // It's up to the arbitrator to create a successor revision containing this claim.
        assert_eq!(event.visibility, EventVisibility::Public);
        assert!(matches!(event.payload, EventPayload::PublicRecord(ref s) if s == answer));
    }

    #[test]
    fn arbitrator_move_missing_provenance_is_rejected() {
        let dissent_id = Uuid::new_v4();
        let base = Revision {
            revision_id: Uuid::new_v4(),
            parent_digest: None,
            content_digest: Digest("dig".into()),
            original_proposal: "Plan".into(),
            current_understanding: "Plan".into(),
            positions: vec![],
            dissents: vec![Dissent {
                id: dissent_id,
                claim: "No".into(),
                resolved_by: None,
            }],
            risks: vec![],
            questions: vec![],
        };

        // A move resolving the dissent, but with no event provenance.
        let output = ArbitrationOutput {
            current_understanding: "Plan".into(),
            moves: vec![crate::revision::Move::ResolveDissent {
                id: dissent_id,
                resolution: Resolution {
                    reason: "Tested".into(),
                    provenance: vec![],
                },
            }],
        };

        let result = apply_arbitration(&base, output);
        assert_eq!(
            result.unwrap_err(),
            "Dissent resolution requires event provenance"
        );
    }

    #[test]
    fn arbitrator_move_batch_applies_and_leaves_untouched_items_unchanged() {
        let dissent_id = Uuid::new_v4();
        let untouched_risk_id = Uuid::new_v4();
        let base = Revision {
            revision_id: Uuid::new_v4(),
            parent_digest: None,
            content_digest: Digest("dig".into()),
            original_proposal: "Plan".into(),
            current_understanding: "Plan".into(),
            positions: vec![],
            dissents: vec![Dissent {
                id: dissent_id,
                claim: "No".into(),
                resolved_by: None,
            }],
            risks: vec![crate::revision::Risk {
                id: untouched_risk_id,
                description: "Untouched risk".into(),
                resolved_by: None,
            }],
            questions: vec![],
        };

        let output = ArbitrationOutput {
            current_understanding: "Updated".into(),
            moves: vec![crate::revision::Move::ResolveDissent {
                id: dissent_id,
                resolution: Resolution {
                    reason: "Tested".into(),
                    provenance: vec![EventRef {
                        event_id: Uuid::new_v4(),
                    }],
                },
            }],
        };

        let successor = apply_arbitration(&base, output).expect("valid move batch applies");
        assert!(successor.dissents[0].resolved_by.is_some());
        // The risk had no move targeting it — it must survive completely unchanged, not
        // inferred as removed or altered.
        assert_eq!(successor.risks[0].id, untouched_risk_id);
        assert!(successor.risks[0].resolved_by.is_none());
        // original_proposal is carried forward unconditionally — there is no field through
        // which the agent could have supplied or altered it.
        assert_eq!(successor.original_proposal, "Plan");
        assert_eq!(successor.current_understanding, "Updated");
    }

    #[test]
    fn condition_evaluator_records_never_reach_respondent() {
        // Evaluate a condition
        let output = ConditionEvaluationOutput {
            verdict: ConditionVerdict::False,
            reason: "Sealed reason: API is down".into(),
            provenance: vec![],
        };

        // This goes into a Sealed event
        let event = Event::new_sealed(
            EventPayload::SealedEvaluation {
                evaluator: "cond".into(),
                reasoning: output.reason.clone(),
            },
            1,
        );
        assert_eq!(event.visibility, EventVisibility::Sealed);

        // When building a respondent prompt from a revision, the sealed event is NOT included
        let base = Revision {
            revision_id: Uuid::new_v4(),
            parent_digest: None,
            content_digest: Digest("dig".into()),
            original_proposal: "Plan".into(),
            current_understanding: "Plan".into(),
            positions: vec![],
            dissents: vec![],
            risks: vec![],
            questions: vec![],
        };

        let prompt = build_respondent_prompt("Question", &base);
        // The prompt must NOT contain the sealed reason
        assert!(!prompt.contains("Sealed reason: API is down"));
    }

    #[test]
    fn persistent_arbitration_session_reconstruction_without_authoritative_memory() {
        use crate::dossier::{ArbitrationSettings, SessionScope, StrategyPreset};
        let settings = ArbitrationSettings::resolve(StrategyPreset::Economy);
        assert_eq!(settings.session_scope, SessionScope::Persistent);

        let id = "session-123".to_string();

        // 1. If we have it in memory, we reuse it.
        let session1 = ArbitrationSession::determine(&settings, Some(id.clone()), false);
        assert_eq!(session1.session_id, id);
        assert_eq!(session1.provenance.reason, "persistent session reused");

        // 2. If we lose memory (None), we just create a new one, proving memory isn't authoritative.
        let session2 = ArbitrationSession::determine(&settings, None, false);
        assert_ne!(session2.session_id, id);
        assert!(session2.session_id.starts_with("session-"));
        assert_eq!(session2.provenance.reason, "process loss reconstruction");

        // 3. For Assurance strategy, we always get a new session even if memory has one.
        let settings_assurance = ArbitrationSettings::resolve(StrategyPreset::Assurance);
        let session3 = ArbitrationSession::determine(&settings_assurance, Some(id.clone()), false);
        assert_ne!(session3.session_id, id);
        assert_eq!(session3.provenance.reason, "fresh per resolution scope");

        // 4. Escalation trigger match forces fresh.
        let session4 = ArbitrationSession::determine(&settings, Some(id.clone()), true);
        assert_ne!(session4.session_id, id);
        assert_eq!(session4.provenance.reason, "escalation trigger matched");
    }
}
