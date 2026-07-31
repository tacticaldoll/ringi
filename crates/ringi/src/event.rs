use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventVisibility {
    Public,
    Sealed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventPayload {
    /// A raw transcript from an agent
    RawTranscript(String),
    /// A sealed evaluation record containing the justification/reasoning
    SealedEvaluation {
        evaluator: String,
        reasoning: String,
    },
    /// A synthesized position or resolution (public)
    Synthesis(String),
    /// A general public record
    PublicRecord(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvocationCoordinate {
    pub dossier_id: Uuid,
    pub role: String,
    pub input_digest: crate::revision::Digest,
    pub turn: u32,
    pub attempt: u32,
}

impl InvocationCoordinate {
    pub fn idempotency_key(&self) -> String {
        format!(
            "{}:{}:{}:{}:{}",
            self.dossier_id, self.role, self.input_digest.0, self.turn, self.attempt
        )
    }
}

/// An append-only event
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    pub id: Uuid,
    pub timestamp: u64, // simplified timestamp
    pub visibility: EventVisibility,
    pub payload: EventPayload,
    pub coordinate: Option<InvocationCoordinate>,
}

impl Event {
    pub fn new_public(payload: EventPayload, timestamp: u64) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp,
            visibility: EventVisibility::Public,
            payload,
            coordinate: None,
        }
    }

    pub fn new_sealed(payload: EventPayload, timestamp: u64) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp,
            visibility: EventVisibility::Sealed,
            payload,
            coordinate: None,
        }
    }
}

/// A projection of the event log tailored for the respondent prompt.
/// Crucially, this projection PROVES that raw transcripts and sealed evaluator records never enter the prompt.
pub struct RespondentContextProjection<'a> {
    events: Vec<&'a Event>,
}

impl<'a> RespondentContextProjection<'a> {
    pub fn build(all_events: &'a [Event]) -> Self {
        let events = all_events
            .iter()
            .filter(|e| e.visibility == EventVisibility::Public)
            .filter(|e| !matches!(e.payload, EventPayload::RawTranscript(_)))
            .filter(|e| !matches!(e.payload, EventPayload::SealedEvaluation { .. }))
            .collect();
        Self { events }
    }

    pub fn visible_events(&self) -> &[&'a Event] {
        &self.events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_respondent_context_projection_excludes_sealed_and_raw() {
        let events = vec![
            Event::new_public(EventPayload::PublicRecord("Started".into()), 1),
            Event::new_public(
                EventPayload::RawTranscript("Some raw agent babble".into()),
                2,
            ),
            Event::new_sealed(
                EventPayload::SealedEvaluation {
                    evaluator: "arbiter".into(),
                    reasoning: "hidden thoughts".into(),
                },
                3,
            ),
            Event::new_public(EventPayload::Synthesis("Clean resolution".into()), 4),
        ];

        let projection = RespondentContextProjection::build(&events);
        let visible = projection.visible_events();

        // Only PublicRecord and Synthesis should be visible
        assert_eq!(visible.len(), 2);
        assert!(matches!(visible[0].payload, EventPayload::PublicRecord(_)));
        assert!(matches!(visible[1].payload, EventPayload::Synthesis(_)));
    }
}
