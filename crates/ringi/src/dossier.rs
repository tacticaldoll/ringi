//! The dossier domain model: `LifecycleState`'s transition machine, the settings a submission
//! locks (`LockedSettings`, `ArbitrationSettings`, `RoleBindings`, `Limits`), draft frontmatter
//! parse/serialize, and `Condition`.
//!
//! `SubmittedDossier::transition_to` is the one place lifecycle transitions are validated — a
//! terminal state accepts no further transition, and only the pairs the state machine names are
//! legal. This module owns no revision content (`revision.rs`) and no persistence (`store.rs`);
//! it is the shape of the dossier itself, checked in memory.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The overall state of a deliberation dossier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleState {
    #[default]
    Draft,
    Submitted,
    Deliberating,
    ReadyForDecision,
    Approved,
    ApprovedWithConditions,
    Rejected,
    Cancelled,
    Invalidated,
}

/// The arbitration strategy preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StrategyPreset {
    #[default]
    Economy,
    Balanced,
    Assurance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Limits {
    pub max_turns: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Self { max_turns: 10 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleBindings {
    pub respondent: String,
    pub arbitrator: String,
}

impl Default for RoleBindings {
    fn default() -> Self {
        Self {
            respondent: "default-agent".to_string(),
            arbitrator: "default-arbitrator".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionScope {
    Persistent,
    FreshPerRound,
    FreshPerResolution,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArbitrationSettings {
    pub preset: StrategyPreset,
    pub session_scope: SessionScope,
    pub pre_decision_review: bool,
    pub escalation_triggers: Vec<String>,
}

impl ArbitrationSettings {
    pub fn resolve(preset: StrategyPreset) -> Self {
        match preset {
            StrategyPreset::Economy => Self {
                preset,
                session_scope: SessionScope::Persistent,
                pre_decision_review: false,
                escalation_triggers: vec![],
            },
            StrategyPreset::Balanced => Self {
                preset,
                session_scope: SessionScope::Persistent,
                pre_decision_review: true,
                escalation_triggers: vec!["low_confidence".into(), "high_severity".into()],
            },
            StrategyPreset::Assurance => Self {
                preset,
                session_scope: SessionScope::FreshPerResolution,
                pre_decision_review: true,
                escalation_triggers: vec!["always".into()],
            },
        }
    }
}

/// Settings that are resolved and locked at submission time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockedSettings {
    pub arbitration: ArbitrationSettings,
    pub limits: Limits,
    pub roles: RoleBindings,
}

/// The frontmatter parsed from a Markdown dossier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Frontmatter {
    pub id: Uuid,
    pub state: LifecycleState,
    #[serde(default)]
    pub strategy: StrategyPreset,
    #[serde(default)]
    pub limits: Limits,
    #[serde(default)]
    pub roles: RoleBindings,
}

impl Frontmatter {
    /// Create a new draft frontmatter with defaults.
    pub fn new_draft() -> Self {
        Self {
            id: Uuid::new_v4(),
            state: LifecycleState::Draft,
            strategy: StrategyPreset::default(),
            limits: Limits::default(),
            roles: RoleBindings::default(),
        }
    }

    /// Submit the draft, locking the settings. Returns an error if not in Draft state.
    pub fn submit(mut self) -> Result<SubmittedDossier, &'static str> {
        if self.state != LifecycleState::Draft {
            return Err("Only a dossier in Draft state can be submitted");
        }
        self.state = LifecycleState::Submitted;
        Ok(SubmittedDossier {
            id: self.id,
            state: self.state,
            locked_settings: LockedSettings {
                arbitration: ArbitrationSettings::resolve(self.strategy),
                limits: self.limits,
                roles: self.roles,
            },
            conditions: vec![],
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Condition {
    pub id: Uuid,
    pub description: String,
    pub is_met: bool,
}

/// A dossier that has been submitted and its settings are locked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmittedDossier {
    pub id: Uuid,
    pub state: LifecycleState,
    pub locked_settings: LockedSettings,
    #[serde(default)]
    pub conditions: Vec<Condition>,
}

impl SubmittedDossier {
    pub fn transition_to(&mut self, next_state: LifecycleState) -> Result<(), &'static str> {
        if matches!(
            self.state,
            LifecycleState::Approved
                | LifecycleState::Rejected
                | LifecycleState::Cancelled
                | LifecycleState::Invalidated
        ) {
            return Err("Cannot transition from a terminal state");
        }

        // Enforce valid transitions
        match (&self.state, &next_state) {
            (LifecycleState::Submitted, LifecycleState::Deliberating) => {}
            (LifecycleState::Deliberating, LifecycleState::ReadyForDecision) => {}
            (LifecycleState::ReadyForDecision, LifecycleState::Deliberating) => {} // back for conditions
            (LifecycleState::ReadyForDecision, LifecycleState::Approved) => {}
            (LifecycleState::ReadyForDecision, LifecycleState::ApprovedWithConditions) => {}
            (LifecycleState::ReadyForDecision, LifecycleState::Rejected) => {}
            (LifecycleState::ApprovedWithConditions, LifecycleState::ReadyForDecision) => {}
            (_, LifecycleState::Cancelled) => {}
            (_, LifecycleState::Invalidated) => {}
            _ => return Err("Invalid state transition"),
        }
        self.state = next_state;
        Ok(())
    }
}

/// Helper to parse and serialize Frontmatter from/to a YAML/JSON block.
pub fn parse_frontmatter(content: &str) -> Result<Frontmatter, serde_json::Error> {
    serde_json::from_str(content)
}

pub fn serialize_frontmatter(fm: &Frontmatter) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(fm)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_draft_to_submit() {
        let draft = Frontmatter::new_draft();
        assert_eq!(draft.state, LifecycleState::Draft);

        let mut submitted = draft.submit().expect("Failed to submit");
        assert_eq!(submitted.state, LifecycleState::Submitted);

        // Cannot submit again
        let not_draft = Frontmatter {
            id: Uuid::new_v4(),
            state: LifecycleState::Submitted,
            strategy: StrategyPreset::Economy,
            limits: Limits::default(),
            roles: RoleBindings::default(),
        };
        assert!(not_draft.submit().is_err());

        submitted
            .transition_to(LifecycleState::Deliberating)
            .unwrap();
        assert_eq!(submitted.state, LifecycleState::Deliberating);
    }

    #[test]
    fn test_roundtrip() {
        let draft = Frontmatter::new_draft();
        let serialized = serialize_frontmatter(&draft).unwrap();
        let parsed = parse_frontmatter(&serialized).unwrap();
        assert_eq!(draft, parsed);
    }

    fn submitted_dossier(state: LifecycleState) -> SubmittedDossier {
        SubmittedDossier {
            id: Uuid::new_v4(),
            state,
            locked_settings: LockedSettings {
                arbitration: ArbitrationSettings::resolve(StrategyPreset::Economy),
                limits: Limits::default(),
                roles: RoleBindings::default(),
            },
            conditions: vec![],
        }
    }

    #[test]
    fn approved_with_conditions_can_reopen_to_ready_for_decision() {
        let mut dossier = submitted_dossier(LifecycleState::ApprovedWithConditions);
        dossier
            .transition_to(LifecycleState::ReadyForDecision)
            .expect("ApprovedWithConditions -> ReadyForDecision is a valid transition");
        assert_eq!(dossier.state, LifecycleState::ReadyForDecision);
    }

    #[test]
    fn a_draft_cannot_be_reopened_to_ready_for_decision() {
        let mut dossier = submitted_dossier(LifecycleState::Submitted);
        let err = dossier
            .transition_to(LifecycleState::ReadyForDecision)
            .unwrap_err();
        assert_eq!(err, "Invalid state transition");
        assert_eq!(dossier.state, LifecycleState::Submitted);
    }
}
