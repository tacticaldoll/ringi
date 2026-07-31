//! Renders a terminal dossier's archive: a human-readable, integrity-bound record of its final
//! SSOT and every recorded event, sealed with a SHA-256 digest over the rendered text.
//!
//! Per `PROJECT.md`'s Archive invariant, this is a record only — it grants no execution
//! authority and triggers no workspace effect. `render_archive` refuses a non-terminal dossier
//! (only `Approved`/`Rejected`/`Cancelled`/`Invalidated` may be archived). Per the Sealed
//! evaluation invariant ("evaluator reasons are archived for humans but never injected into
//! respondent or synthesis context"), the archive is where sealed reasoning surfaces for a human
//! — never fed back into `deliberation.rs`'s prompt builders.

use crate::dossier::LifecycleState;
use crate::event::{Event, EventPayload, EventVisibility};
use crate::store::DossierStore;
use anyhow::Context;
use sha2::{Digest, Sha256};
use std::fmt::Write;

/// One human-readable line for an event's content, regardless of which payload variant it is —
/// so a rendered section never silently drops a payload kind no current caller constructs yet
/// (`RawTranscript`/`Synthesis`).
fn render_event_line(event: &Event) -> String {
    match &event.payload {
        EventPayload::RawTranscript(text) => format!("Raw transcript: {text}"),
        EventPayload::Synthesis(text) => format!("Synthesis: {text}"),
        EventPayload::PublicRecord(text) => text.clone(),
        EventPayload::SealedEvaluation {
            evaluator,
            reasoning,
        } => format!("[{evaluator}] {reasoning}"),
    }
}

pub fn render_archive(dossier_id: &str, store: &DossierStore) -> anyhow::Result<String> {
    let state_json = store
        .get_dossier_state(dossier_id)?
        .context("Dossier not found")?;
    let dossier: crate::dossier::SubmittedDossier = serde_json::from_str(&state_json)?;

    if !matches!(
        dossier.state,
        LifecycleState::Approved
            | LifecycleState::Rejected
            | LifecycleState::Cancelled
            | LifecycleState::Invalidated
    ) {
        anyhow::bail!("Cannot archive a non-terminal dossier");
    }

    let mut out = String::new();
    writeln!(&mut out, "# Dossier Archive: {}", dossier_id)?;
    writeln!(&mut out, "\n**State**: {:?}", dossier.state)?;
    writeln!(
        &mut out,
        "**Strategy**: {:?}",
        dossier.locked_settings.arbitration.preset
    )?;

    let latest_revision = store.get_latest_revision(dossier_id)?;

    if let Some(rev) = &latest_revision {
        writeln!(&mut out, "\n## Final SSOT")?;
        writeln!(&mut out, "\n### Original Proposal")?;
        writeln!(&mut out, "{}", rev.original_proposal)?;
        writeln!(&mut out, "\n### Final Understanding")?;
        writeln!(&mut out, "{}", rev.current_understanding)?;
    } else {
        writeln!(&mut out, "\n*(No revisions found)*")?;
    }

    writeln!(&mut out, "\n## Conditions")?;
    let conditions = latest_revision
        .as_ref()
        .map(|rev| rev.conditions.as_slice())
        .unwrap_or(&[]);
    if conditions.is_empty() {
        writeln!(&mut out, "*(No conditions attached)*")?;
    } else {
        for condition in conditions {
            writeln!(
                &mut out,
                "- [{}] {}",
                if condition.resolved_by.is_some() {
                    "x"
                } else {
                    " "
                },
                condition.description
            )?;
        }
    }

    let events = store.events_for_dossier(dossier_id)?;

    writeln!(&mut out, "\n## Public Event Index")?;
    let public: Vec<&Event> = events
        .iter()
        .filter(|e| e.visibility == EventVisibility::Public)
        .collect();
    if public.is_empty() {
        writeln!(&mut out, "*(No public events recorded)*")?;
    } else {
        for event in public {
            writeln!(&mut out, "- {}", render_event_line(event))?;
        }
    }

    writeln!(&mut out, "\n## Sealed Audit Section")?;
    let sealed: Vec<&Event> = events
        .iter()
        .filter(|e| e.visibility == EventVisibility::Sealed)
        .collect();
    if sealed.is_empty() {
        writeln!(&mut out, "*(No sealed evaluations recorded)*")?;
    } else {
        for event in sealed {
            writeln!(&mut out, "- {}", render_event_line(event))?;
        }
    }

    // Compute integrity digest
    let mut hasher = Sha256::new();
    hasher.update(out.as_bytes());
    let digest_bytes = hasher.finalize();
    let digest: String = digest_bytes.iter().map(|b| format!("{:02x}", b)).collect();

    writeln!(
        &mut out,
        "\n---\n**Integrity Digest (SHA-256)**: {}",
        digest
    )?;

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dossier::{
        ArbitrationSettings, Limits, LockedSettings, RoleBindings, StrategyPreset, SubmittedDossier,
    };
    use crate::revision::{Condition, Digest as RevisionDigest, Revision};
    use uuid::Uuid;

    fn approved_dossier(id: Uuid) -> SubmittedDossier {
        SubmittedDossier {
            id,
            state: LifecycleState::Approved,
            locked_settings: LockedSettings {
                arbitration: ArbitrationSettings::resolve(StrategyPreset::Economy),
                limits: Limits { max_turns: 1 },
                roles: RoleBindings {
                    respondent: "unused".to_string(),
                    arbitrator: "unused".to_string(),
                },
            },
        }
    }

    fn base_revision() -> Revision {
        revision_with_conditions(vec![])
    }

    fn revision_with_conditions(conditions: Vec<Condition>) -> Revision {
        Revision {
            revision_id: Uuid::new_v4(),
            parent_digest: None,
            content_digest: RevisionDigest("dig".into()),
            original_proposal: "p".into(),
            current_understanding: "u".into(),
            positions: vec![],
            dissents: vec![],
            risks: vec![],
            questions: vec![],
            conditions,
        }
    }

    fn open_store(name: &str) -> DossierStore {
        let path = std::env::temp_dir().join(format!(
            "ringi-archive-{name}-{}.sqlite",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        DossierStore::open(&path).unwrap()
    }

    #[test]
    fn public_events_render_in_commit_order() {
        let mut store = open_store("public-order");
        let id = Uuid::new_v4();
        let id_str = id.to_string();
        store
            .insert_dossier(
                &id_str,
                &serde_json::to_string(&approved_dossier(id)).unwrap(),
            )
            .unwrap();

        let first = Event::new_public(EventPayload::PublicRecord("first claim".into()), 1);
        let second = Event::new_public(EventPayload::PublicRecord("second claim".into()), 2);
        store
            .commit_successor_revision(&id_str, None, &base_revision(), &[first, second])
            .unwrap();

        let rendered = render_archive(&id_str, &store).unwrap();
        let first_pos = rendered.find("first claim").unwrap();
        let second_pos = rendered.find("second claim").unwrap();
        assert!(first_pos < second_pos, "events must render in commit order");
    }

    #[test]
    fn a_sealed_evaluation_renders_its_evaluator_and_reasoning() {
        let mut store = open_store("sealed");
        let id = Uuid::new_v4();
        let id_str = id.to_string();
        store
            .insert_dossier(
                &id_str,
                &serde_json::to_string(&approved_dossier(id)).unwrap(),
            )
            .unwrap();

        let sealed = Event::new_sealed(
            EventPayload::SealedEvaluation {
                evaluator: "condition:budget".into(),
                reasoning: "under the cap".into(),
            },
            1,
        );
        store
            .commit_successor_revision(&id_str, None, &base_revision(), &[sealed])
            .unwrap();

        let rendered = render_archive(&id_str, &store).unwrap();
        assert!(rendered.contains("[condition:budget] under the cap"));
    }

    #[test]
    fn empty_sections_render_an_explicit_placeholder() {
        let mut store = open_store("empty");
        let id = Uuid::new_v4();
        let id_str = id.to_string();
        store
            .insert_dossier(
                &id_str,
                &serde_json::to_string(&approved_dossier(id)).unwrap(),
            )
            .unwrap();
        store
            .commit_successor_revision(&id_str, None, &base_revision(), &[])
            .unwrap();

        let rendered = render_archive(&id_str, &store).unwrap();
        assert!(rendered.contains("*(No public events recorded)*"));
        assert!(rendered.contains("*(No sealed evaluations recorded)*"));
        assert!(rendered.contains("*(No conditions attached)*"));
    }

    #[test]
    fn conditions_render_as_checkboxes_matching_their_final_resolved_status() {
        let mut store = open_store("conditions");
        let id = Uuid::new_v4();
        let id_str = id.to_string();
        store
            .insert_dossier(
                &id_str,
                &serde_json::to_string(&approved_dossier(id)).unwrap(),
            )
            .unwrap();
        let revision = revision_with_conditions(vec![
            Condition {
                id: Uuid::new_v4(),
                description: "Security review completed".into(),
                resolved_by: Some(crate::revision::Resolution {
                    reason: "reviewed".into(),
                    provenance: vec![],
                }),
            },
            Condition {
                id: Uuid::new_v4(),
                description: "Load test passed".into(),
                resolved_by: None,
            },
        ]);
        store
            .commit_successor_revision(&id_str, None, &revision, &[])
            .unwrap();

        let rendered = render_archive(&id_str, &store).unwrap();
        assert!(rendered.contains("- [x] Security review completed"));
        assert!(rendered.contains("- [ ] Load test passed"));
    }

    #[test]
    fn the_integrity_digest_differs_when_event_content_differs() {
        let mut store_a = open_store("digest-a");
        let id_a = Uuid::new_v4();
        let id_a_str = id_a.to_string();
        store_a
            .insert_dossier(
                &id_a_str,
                &serde_json::to_string(&approved_dossier(id_a)).unwrap(),
            )
            .unwrap();
        store_a
            .commit_successor_revision(
                &id_a_str,
                None,
                &base_revision(),
                &[Event::new_public(
                    EventPayload::PublicRecord("claim A".into()),
                    1,
                )],
            )
            .unwrap();

        let mut store_b = open_store("digest-b");
        let id_b = Uuid::new_v4();
        let id_b_str = id_b.to_string();
        store_b
            .insert_dossier(
                &id_b_str,
                &serde_json::to_string(&approved_dossier(id_b)).unwrap(),
            )
            .unwrap();
        store_b
            .commit_successor_revision(
                &id_b_str,
                None,
                &base_revision(),
                &[Event::new_public(
                    EventPayload::PublicRecord("claim B".into()),
                    1,
                )],
            )
            .unwrap();

        let rendered_a = render_archive(&id_a_str, &store_a).unwrap();
        let rendered_b = render_archive(&id_b_str, &store_b).unwrap();

        let digest_of = |rendered: &str| {
            rendered
                .lines()
                .find(|l| l.starts_with("**Integrity Digest"))
                .unwrap()
                .to_string()
        };
        assert_ne!(digest_of(&rendered_a), digest_of(&rendered_b));
    }
}
