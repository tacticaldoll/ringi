//! Thin CLI command wiring: each `pub fn ..._command` is one `ringi` subcommand, translating
//! between the on-disk draft markdown file (frontmatter + body, under `.ringi/dossiers/`), the
//! durable `DossierStore`, and the domain modules (`dossier`, `revision`, `deliberate_loop`,
//! `archive`). It owns no domain logic of its own — a command either delegates to a domain
//! function or does the minimal glue (parsing frontmatter, computing a path) a human-facing CLI
//! needs that the domain modules should not need to know about.

use crate::dossier::{
    Condition, Frontmatter, LifecycleState, parse_frontmatter, serialize_frontmatter,
};
use crate::store::DossierStore;
use anyhow::{Context, bail};
use std::path::{Path, PathBuf};

fn dossiers_dir() -> PathBuf {
    Path::new(".ringi").join("dossiers")
}

pub fn draft_command() -> anyhow::Result<()> {
    let dir = dossiers_dir();
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;

    let draft = Frontmatter::new_draft();
    let id = draft.id.to_string();
    let path = dir.join(format!("{}.md", id));

    let content = format!(
        "---\n{}---\n\n# Propose Action Here\n\nWrite your intent...\n",
        serialize_frontmatter(&draft)?
    );

    std::fs::write(&path, content).with_context(|| format!("writing {}", path.display()))?;
    println!("Created draft dossier at {}", path.display());
    Ok(())
}

pub fn submit_command(id: &str, store: &mut DossierStore) -> anyhow::Result<()> {
    let path = dossiers_dir().join(format!("{}.md", id));
    if !path.exists() {
        bail!("Dossier file {} not found", path.display());
    }

    let content = std::fs::read_to_string(&path)?;
    let parts: Vec<&str> = content.splitn(3, "---").collect();
    if parts.len() < 3 {
        bail!("Dossier file missing frontmatter");
    }

    let mut frontmatter = parse_frontmatter(parts[1])?;
    if frontmatter.id.to_string() != id {
        bail!("Dossier ID mismatch in frontmatter");
    }

    let submitted = frontmatter
        .clone()
        .submit()
        .map_err(|e| anyhow::anyhow!(e))?;

    // Update the frontmatter state
    frontmatter.state = LifecycleState::Submitted;
    let new_content = format!(
        "---\n{}---{}",
        serialize_frontmatter(&frontmatter)?,
        parts[2]
    );
    std::fs::write(&path, new_content)?;

    // Commit to SQLite
    let state_json = serde_json::to_string(&submitted)?;
    store.insert_dossier(id, &state_json)?;

    // Create initial revision from the dossier body
    let mut initial_revision = crate::revision::Revision {
        revision_id: uuid::Uuid::new_v4(),
        parent_digest: None,
        content_digest: crate::revision::Digest(String::new()),
        original_proposal: parts[2].trim().to_string(),
        current_understanding: parts[2].trim().to_string(),
        positions: vec![],
        dissents: vec![],
        risks: vec![],
    };
    initial_revision.content_digest = initial_revision.compute_digest();
    store.commit_successor_revision(id, None, &initial_revision, &[])?;

    println!("Submitted dossier {}", id);
    Ok(())
}

pub fn continue_command(
    id: &str,
    store: &mut DossierStore,
    registry: &crate::registry::SqliteRegistry,
) -> anyhow::Result<()> {
    let state_json = store
        .get_dossier_state(id)?
        .context("Dossier not found in store")?;
    // The Submitted -> Deliberating transition is owned by run_deliberation.
    crate::deliberate_loop::run_deliberation(id, &state_json, store, registry)
}

pub fn evaluate_command(
    id: &str,
    store: &mut DossierStore,
    registry: &crate::registry::SqliteRegistry,
) -> anyhow::Result<()> {
    let state_json = store
        .get_dossier_state(id)?
        .context("Dossier not found in store")?;
    crate::deliberate_loop::evaluate_conditions(id, &state_json, store, registry)
}

pub fn transition_command(
    id: &str,
    target_state: LifecycleState,
    store: &mut DossierStore,
) -> anyhow::Result<()> {
    let state_json = store
        .get_dossier_state(id)?
        .context("Dossier not found in store")?;
    let mut dossier: crate::dossier::SubmittedDossier = serde_json::from_str(&state_json)?;

    // Special handling for ApprovedWithConditions
    let next_state = if target_state == LifecycleState::Approved && !dossier.conditions.is_empty() {
        // If there are conditions that are NOT met, we go to ApprovedWithConditions
        if dossier.conditions.iter().any(|c| !c.is_met) {
            LifecycleState::ApprovedWithConditions
        } else {
            target_state
        }
    } else {
        target_state
    };

    dossier
        .transition_to(next_state)
        .map_err(|e| anyhow::anyhow!(e))?;

    let new_state_json = serde_json::to_string(&dossier)?;
    store.insert_dossier(id, &new_state_json)?;

    // Update markdown frontmatter
    let path = dossiers_dir().join(format!("{}.md", id));
    if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        let parts: Vec<&str> = content.splitn(3, "---").collect();
        if parts.len() == 3 {
            let mut frontmatter = parse_frontmatter(parts[1])?;
            frontmatter.state = dossier.state;
            let new_content = format!(
                "---\n{}---{}",
                serialize_frontmatter(&frontmatter)?,
                parts[2]
            );
            std::fs::write(&path, new_content)?;
        }
    }

    println!("Decision recorded for dossier {}: {:?}", id, dossier.state);

    // If terminal, generate archive
    if matches!(
        dossier.state,
        LifecycleState::Approved
            | LifecycleState::Rejected
            | LifecycleState::Cancelled
            | LifecycleState::Invalidated
    ) {
        let archive_content = crate::archive::render_archive(id, store)?;
        let archive_path = dossiers_dir().join(format!("{}.archive.md", id));
        std::fs::write(&archive_path, archive_content)?;
        println!("Archive generated at {}", archive_path.display());
    }

    Ok(())
}

pub fn add_condition_command(
    id: &str,
    description: &str,
    store: &mut DossierStore,
) -> anyhow::Result<()> {
    let state_json = store
        .get_dossier_state(id)?
        .context("Dossier not found in store")?;
    let mut dossier: crate::dossier::SubmittedDossier = serde_json::from_str(&state_json)?;

    if dossier.state != LifecycleState::ReadyForDecision {
        bail!("Can only add conditions to a dossier in ReadyForDecision state");
    }

    let condition = Condition {
        id: uuid::Uuid::new_v4(),
        description: description.to_string(),
        is_met: false,
    };

    dossier.conditions.push(condition);
    let new_state_json = serde_json::to_string(&dossier)?;
    store.insert_dossier(id, &new_state_json)?;

    println!("Added condition to dossier {}: {}", id, description);
    Ok(())
}

/// Whether `state` is one of the states for which "is this ready for a decision" is still a
/// live question — false once a decision has already been rendered (a terminal state).
fn is_decision_pending(state: &LifecycleState) -> bool {
    matches!(
        state,
        LifecycleState::Draft
            | LifecycleState::Submitted
            | LifecycleState::Deliberating
            | LifecycleState::ReadyForDecision
    )
}

pub fn inspect_command(id: &str, store: &DossierStore) -> anyhow::Result<()> {
    let state_json = store
        .get_dossier_state(id)?
        .context("Dossier not found in store")?;
    let dossier: crate::dossier::SubmittedDossier = serde_json::from_str(&state_json)?;

    println!("Dossier ID: {}", dossier.id);
    println!("State: {:?}", dossier.state);

    if let Some(rev) = store.get_latest_revision(id)? {
        println!("Latest Revision: {}", rev.revision_id);
        // Readiness is a mechanical fact recomputed from the residual, never a stored flag.
        // Matches run_deliberation's own rule: an un-deliberated root is never ready on its own.
        // Once a decision has been rendered (a terminal state), readiness is no longer a live
        // question, so the line is omitted rather than shown alongside an already-settled state.
        if is_decision_pending(&dossier.state) {
            println!(
                "Readiness: {}",
                crate::convergence::is_ready_for_decision(&rev)
            );
        }
    }

    if !dossier.conditions.is_empty() {
        println!("\nConditions:");
        for c in &dossier.conditions {
            println!("- [{}] {}", if c.is_met { "x" } else { " " }, c.description);
        }
    }

    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::dossier::{
        ArbitrationSettings, Limits, LockedSettings, RoleBindings, StrategyPreset, SubmittedDossier,
    };
    use crate::revision::{Digest, Revision};
    use std::os::unix::fs::PermissionsExt;
    use uuid::Uuid;

    fn fake_agent(name: &str, body: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ringi-cli-agent-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    // Mutates the process's current directory for the duration of the test; see
    // `crate::PROCESS_CWD_LOCK`.
    #[test]
    fn test_submit_computes_a_real_content_digest() {
        let _guard = crate::PROCESS_CWD_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());

        let dir = std::env::temp_dir().join(format!("ringi-submit-cli-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let original_cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        let draft = Frontmatter::new_draft();
        let id = draft.id.to_string();
        std::fs::create_dir_all(dossiers_dir()).unwrap();
        let content = format!(
            "---\n{}---\n\nSome proposal body\n",
            serialize_frontmatter(&draft).unwrap()
        );
        std::fs::write(dossiers_dir().join(format!("{}.md", id)), content).unwrap();

        let mut store = DossierStore::open(dir.join("store.sqlite")).unwrap();
        let result = submit_command(&id, &mut store);

        std::env::set_current_dir(&original_cwd).unwrap();
        let _ = std::fs::remove_dir_all(&dir);

        result.unwrap();
        let revision = store.get_latest_revision(&id).unwrap().unwrap();
        assert_ne!(revision.content_digest.0, "initial-digest");
        assert_eq!(revision.content_digest, revision.compute_digest());
    }

    // Mutates the process's current directory for the duration of the test; see
    // `crate::PROCESS_CWD_LOCK`.
    #[test]
    fn a_dossier_reaches_approved_once_evaluate_satisfies_its_only_condition() {
        let _guard = crate::PROCESS_CWD_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());

        let dir = std::env::temp_dir().join(format!("ringi-approve-cli-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // transition_command writes an archive under `.ringi/dossiers` on reaching Approved.
        std::fs::create_dir_all(dir.join(".ringi").join("dossiers")).unwrap();

        let evaluator = fake_agent(
            "eval-true.sh",
            "echo '{\"verdict\":\"True\",\"reason\":\"under budget\",\"provenance\":[]}'",
        );

        let id = Uuid::new_v4();
        let id_str = id.to_string();
        let dossier = SubmittedDossier {
            id,
            state: LifecycleState::ReadyForDecision,
            locked_settings: LockedSettings {
                arbitration: ArbitrationSettings::resolve(StrategyPreset::Economy),
                limits: Limits { max_turns: 1 },
                roles: RoleBindings {
                    respondent: evaluator.to_str().unwrap().to_string(),
                    arbitrator: "unused".to_string(),
                },
            },
            conditions: vec![Condition {
                id: Uuid::new_v4(),
                description: "Budget is under $1000".into(),
                is_met: false,
            }],
        };
        let json = serde_json::to_string(&dossier).unwrap();

        let store_path = dir.join("store.sqlite");
        let mut store = DossierStore::open(&store_path).unwrap();
        let registry = crate::registry::SqliteRegistry::open(&store_path).unwrap();
        store.insert_dossier(&id_str, &json).unwrap();
        store
            .commit_successor_revision(
                &id_str,
                None,
                &Revision {
                    revision_id: Uuid::new_v4(),
                    parent_digest: None,
                    content_digest: Digest("dig".into()),
                    original_proposal: "p".into(),
                    current_understanding: "u".into(),
                    positions: vec![],
                    dissents: vec![],
                    risks: vec![],
                },
                &[],
            )
            .unwrap();

        let original_cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();
        let result = (|| -> anyhow::Result<()> {
            evaluate_command(&id_str, &mut store, &registry)?;
            transition_command(&id_str, LifecycleState::Approved, &mut store)
        })();
        std::env::set_current_dir(&original_cwd).unwrap();
        let _ = std::fs::remove_dir_all(&dir);

        result.unwrap();
        let state_json = store.get_dossier_state(&id_str).unwrap().unwrap();
        let updated: SubmittedDossier = serde_json::from_str(&state_json).unwrap();
        assert_eq!(updated.state, LifecycleState::Approved);
    }

    #[test]
    fn reopening_an_approved_with_conditions_dossier_lets_it_reach_approved() {
        let _guard = crate::PROCESS_CWD_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());

        let dir = std::env::temp_dir().join(format!("ringi-reopen-cli-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::create_dir_all(dir.join(".ringi").join("dossiers")).unwrap();

        let evaluator = fake_agent(
            "eval-true.sh",
            "echo '{\"verdict\":\"True\",\"reason\":\"signed off\",\"provenance\":[]}'",
        );

        let id = Uuid::new_v4();
        let id_str = id.to_string();
        // Starts already ApprovedWithConditions — the state a real dossier reaches after
        // `approve` finds an unmet condition (transition_command's own routing).
        let dossier = SubmittedDossier {
            id,
            state: LifecycleState::ApprovedWithConditions,
            locked_settings: LockedSettings {
                arbitration: ArbitrationSettings::resolve(StrategyPreset::Economy),
                limits: Limits { max_turns: 1 },
                roles: RoleBindings {
                    respondent: evaluator.to_str().unwrap().to_string(),
                    arbitrator: "unused".to_string(),
                },
            },
            conditions: vec![Condition {
                id: Uuid::new_v4(),
                description: "Security team has signed off".into(),
                is_met: false,
            }],
        };
        let json = serde_json::to_string(&dossier).unwrap();

        let store_path = dir.join("store.sqlite");
        let mut store = DossierStore::open(&store_path).unwrap();
        let registry = crate::registry::SqliteRegistry::open(&store_path).unwrap();
        store.insert_dossier(&id_str, &json).unwrap();
        store
            .commit_successor_revision(
                &id_str,
                None,
                &Revision {
                    revision_id: Uuid::new_v4(),
                    parent_digest: None,
                    content_digest: Digest("dig".into()),
                    original_proposal: "p".into(),
                    current_understanding: "u".into(),
                    positions: vec![],
                    dissents: vec![],
                    risks: vec![],
                },
                &[],
            )
            .unwrap();

        let original_cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();
        let result = (|| -> anyhow::Result<()> {
            // The reopen path: ApprovedWithConditions -> ReadyForDecision.
            transition_command(&id_str, LifecycleState::ReadyForDecision, &mut store)?;
            evaluate_command(&id_str, &mut store, &registry)?;
            transition_command(&id_str, LifecycleState::Approved, &mut store)
        })();
        std::env::set_current_dir(&original_cwd).unwrap();
        let _ = std::fs::remove_dir_all(&dir);

        result.unwrap();
        let state_json = store.get_dossier_state(&id_str).unwrap().unwrap();
        let updated: SubmittedDossier = serde_json::from_str(&state_json).unwrap();
        assert_eq!(updated.state, LifecycleState::Approved);
    }

    #[test]
    fn readiness_is_a_live_question_only_before_a_decision_is_rendered() {
        let pending = [
            LifecycleState::Draft,
            LifecycleState::Submitted,
            LifecycleState::Deliberating,
            LifecycleState::ReadyForDecision,
        ];
        let terminal = [
            LifecycleState::Approved,
            LifecycleState::ApprovedWithConditions,
            LifecycleState::Rejected,
            LifecycleState::Cancelled,
            LifecycleState::Invalidated,
        ];

        for state in &pending {
            assert!(
                is_decision_pending(state),
                "{:?} should still treat readiness as a live question",
                state
            );
        }
        for state in &terminal {
            assert!(
                !is_decision_pending(state),
                "{:?} has already rendered a decision; readiness is no longer live",
                state
            );
        }
    }
}
