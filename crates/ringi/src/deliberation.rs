use crate::revision::Revision;

/// Build the prompt for a respondent agent.
/// It contains the original proposal, current public revision state (understanding, positions),
/// unresolved items (dissents, risks), and the specific question to answer.
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

    let unresolved_risks = &revision.unresolved_risks;
    if !unresolved_risks.is_empty() {
        prompt.push_str("\n## Unresolved Risks\n");
        for r in unresolved_risks {
            prompt.push_str(&format!("- {}\n", r));
        }
    }

    prompt.push_str("\n## Question for you\n");
    prompt.push_str(question);
    prompt.push_str("\n\nPlease provide your answer.");

    prompt
}

/// The structured output expected from an arbitration session.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ArbitrationOutput {
    /// A complete successor SSOT revision replacing the previous.
    pub successor_revision: Revision,
    /// A list of zero or more specific questions for respondents.
    pub next_questions: Vec<String>,
    /// A readiness boolean indicating whether the revision is ready for human approval.
    pub readiness: bool,
}

/// Applies an arbitration output to a base revision, enforcing structural validity.
/// Returns the new successor revision, the next questions, and the readiness flag.
pub fn apply_arbitration(
    base: &Revision,
    output: ArbitrationOutput,
) -> Result<(Revision, Vec<String>, bool), &'static str> {
    let mut successor = output.successor_revision;
    // Readiness from the output must match the revision's readiness, or we override it.
    successor.readiness = output.readiness;
    let validated_successor = base.propose_successor(successor)?;
    Ok((validated_successor, output.next_questions, output.readiness))
}

/// Build the prompt for an arbitrator agent.
/// It contains the full history (simplified as the current revision for now),
/// unresolved items, and recent respondent claims (passed as events).
pub fn build_arbitrator_prompt(revision: &Revision, recent_claims: &[String]) -> String {
    let mut prompt = String::new();
    prompt.push_str("You are the arbitrator.\n\n");
    prompt.push_str("## Current SSOT\n");
    prompt.push_str(&revision.current_understanding);
    prompt.push('\n');

    if !recent_claims.is_empty() {
        prompt.push_str("\n## Recent Respondent Claims\n");
        for claim in recent_claims {
            prompt.push_str(&format!("- {}\n", claim));
        }
    }

    prompt
        .push_str("\nPlease propose a complete successor revision, next questions, and readiness.");
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Event, EventPayload, EventVisibility};
    use crate::revision::{Digest, Dissent};
    use uuid::Uuid;

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
            unresolved_risks: vec!["Aliens".into()],
            readiness: false,
        };

        let prompt = build_respondent_prompt("What fuel to use?", &revision);
        assert!(prompt.contains("Let's build a spaceship"));
        assert!(prompt.contains("Mars is cool"));
        assert!(prompt.contains("Too expensive"));
        assert!(!prompt.contains("Too far")); // resolved should not be included
        assert!(prompt.contains("Aliens"));
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
    fn arbitrator_parses_and_validates_proposal() {
        let base = Revision {
            revision_id: Uuid::new_v4(),
            parent_digest: None,
            content_digest: Digest("dig".into()),
            original_proposal: "Plan".into(),
            current_understanding: "Plan".into(),
            positions: vec![],
            dissents: vec![Dissent {
                id: Uuid::new_v4(),
                claim: "No".into(),
                resolved_by: None,
            }],
            unresolved_risks: vec![],
            readiness: false,
        };

        // Missing dissent resolution
        let output = ArbitrationOutput {
            successor_revision: Revision {
                revision_id: Uuid::new_v4(),
                parent_digest: None,
                content_digest: Digest("".into()),
                original_proposal: "Plan".into(),
                current_understanding: "Plan".into(),
                positions: vec![],
                dissents: vec![], // Dropped dissent!
                unresolved_risks: vec![],
                readiness: true,
            },
            next_questions: vec![],
            readiness: true,
        };

        let result = apply_arbitration(&base, output);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Cannot silently remove an unresolved dissent"
        );
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
            unresolved_risks: vec![],
            readiness: false,
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
