use anyhow::{Context, bail};
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

use crate::agent::{AgentAdapter, AgentRequest, AgentRole, SubprocessAdapter};
use crate::deliberation::{
    ArbitrationOutput, apply_arbitration, build_arbitrator_prompt, build_respondent_prompt,
};
use crate::dossier::{LifecycleState, SubmittedDossier};
use crate::store::DossierStore;

pub fn run_deliberation(
    dossier_id: &str,
    dossier_json: &str,
    store: &mut DossierStore,
) -> anyhow::Result<()> {
    let mut dossier: SubmittedDossier = serde_json::from_str(dossier_json)?;

    if dossier.state != LifecycleState::Deliberating && dossier.state != LifecycleState::Submitted {
        bail!(
            "Dossier {} is not deliberating (state: {:?})",
            dossier_id,
            dossier.state
        );
    }

    // A submitted dossier enters deliberation before the first turn.
    if dossier.state == LifecycleState::Submitted {
        dossier
            .transition_to(LifecycleState::Deliberating)
            .map_err(|e| anyhow::anyhow!(e))?;
        store.insert_dossier(dossier_id, &serde_json::to_string(&dossier)?)?;
    }

    // A deliberation loop starts from the latest revision of the dossier.
    let mut current_revision = match store.get_latest_revision(dossier_id)? {
        Some(r) => r,
        None => {
            bail!(
                "No revisions found in dossier {} - cannot deliberate without an initial revision",
                dossier_id
            );
        }
    };

    // Readiness is a mechanical fact computed from the residual by suunta, never an agent
    // claim; it is evaluated on every freshly-produced successor below so final-turn
    // convergence still transitions. Here, before any turn, only a revision produced by
    // arbitration (one with a parent) may already be converged — this covers resuming an
    // already-converged dossier. The un-deliberated root is never treated as converged: an
    // empty residual there means "not yet deliberated", not "resolved".
    if current_revision.parent_digest.is_some() && crate::convergence::is_ready(&current_revision) {
        return mark_ready(dossier_id, &mut dossier, store);
    }

    // Own the settings we need so no borrow of `dossier` is held across `mark_ready`.
    let max_turns = dossier.locked_settings.limits.max_turns;
    let respondent_program = dossier.locked_settings.roles.respondent.clone();
    let arbitrator_program = dossier.locked_settings.roles.arbitrator.clone();

    let mut turn = 1;
    while turn <= max_turns {
        println!("--- Turn {} ---", turn);

        let question = "Please review the unresolved dissents and risks and provide a claim on how to proceed.".to_string();
        let respondent_prompt = build_respondent_prompt(&question, &current_revision);

        let respondent = SubprocessAdapter::new(respondent_program.clone(), vec![]);
        let req = AgentRequest {
            role: AgentRole::Respondent,
            session_instruction: None,
            prompt: respondent_prompt,
            working_dir: std::env::current_dir()?,
            timeout: Duration::from_secs(60),
            env: HashMap::new(),
        };

        println!("Turn {}: Invoking respondent...", turn);
        let res = respondent.run(req)?;
        if res.exit_code != Some(0) {
            bail!("Respondent failed: {}", res.stderr);
        }

        let claim = res.stdout.trim().to_string();
        println!("Turn {}: Respondent answered with claim: {}", turn, claim);

        // Record respondent event
        let mut respondent_event = crate::event::Event::new_public(
            crate::event::EventPayload::PublicRecord(claim.clone()),
            turn as u64 * 1000,
        );
        respondent_event.coordinate = Some(crate::event::InvocationCoordinate {
            dossier_id: Uuid::parse_str(dossier_id).unwrap_or_default(),
            role: "respondent".to_string(),
            input_digest: current_revision.content_digest.clone(),
            turn,
            attempt: 1,
        });

        println!("Turn {}: Building arbitrator prompt...", turn);
        let arbitrator_prompt = build_arbitrator_prompt(&current_revision, &[claim]);

        let arbitrator = SubprocessAdapter::new(arbitrator_program.clone(), vec![]);
        let arb_agent_req = AgentRequest {
            role: AgentRole::Arbitrator,
            session_instruction: None,
            prompt: arbitrator_prompt,
            working_dir: std::env::current_dir()?,
            timeout: Duration::from_secs(60),
            env: HashMap::new(),
        };

        println!("Turn {}: Invoking arbitrator...", turn);
        let arb_res = arbitrator.run(arb_agent_req)?;
        if arb_res.exit_code != Some(0) {
            bail!("Arbitrator failed: {}", arb_res.stderr);
        }

        let metadata = arb_res
            .metadata
            .context("Arbitrator produced no structured output")?;
        let resolution_output: ArbitrationOutput = serde_json::from_value(metadata)?;

        println!("Turn {}: Applying arbitration...", turn);
        let (mut successor, _next_questions) =
            apply_arbitration(&current_revision, resolution_output)
                .map_err(|e| anyhow::anyhow!(e))?;

        successor.revision_id = Uuid::new_v4();

        let events = vec![respondent_event];

        // Commit the successor revision atomically with the events.
        store.commit_successor_revision(
            dossier_id,
            Some(&current_revision.revision_id.to_string()),
            &successor,
            &events,
        )?;

        current_revision = successor;

        // Evaluate convergence on the freshly-produced successor.
        if crate::convergence::is_ready(&current_revision) {
            return mark_ready(dossier_id, &mut dossier, store);
        }

        turn += 1;
    }

    println!(
        "Dossier {} reached max turns ({}) without convergence.",
        dossier_id, max_turns
    );

    Ok(())
}

/// Transition a dossier to `ReadyForDecision` and persist it.
fn mark_ready(
    dossier_id: &str,
    dossier: &mut SubmittedDossier,
    store: &mut DossierStore,
) -> anyhow::Result<()> {
    println!("Dossier {} is ready for decision.", dossier_id);
    dossier
        .transition_to(LifecycleState::ReadyForDecision)
        .map_err(|e| anyhow::anyhow!(e))?;
    store.insert_dossier(dossier_id, &serde_json::to_string(dossier)?)?;
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::dossier::{
        ArbitrationSettings, Limits, LockedSettings, RoleBindings, StrategyPreset, SubmittedDossier,
    };
    use crate::revision::{Digest, Dissent, Revision};
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn fake_agent(name: &str, body: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ringi-loop-agent-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn submitted(id: Uuid, respondent: &str, arbitrator: &str, max_turns: u32) -> SubmittedDossier {
        SubmittedDossier {
            id,
            state: LifecycleState::Submitted,
            locked_settings: LockedSettings {
                arbitration: ArbitrationSettings::resolve(StrategyPreset::Economy),
                limits: Limits { max_turns },
                roles: RoleBindings {
                    respondent: respondent.to_string(),
                    arbitrator: arbitrator.to_string(),
                },
            },
            conditions: vec![],
        }
    }

    fn state_of(store: &DossierStore, id: &str) -> LifecycleState {
        let json = store.get_dossier_state(id).unwrap().unwrap();
        let d: SubmittedDossier = serde_json::from_str(&json).unwrap();
        d.state
    }

    #[test]
    fn a_deliberated_converged_revision_transitions_on_resume() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("ringi-loop-resume-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);

        let id = Uuid::new_v4();
        let id_str = id.to_string();
        let mut store = DossierStore::open(&path).unwrap();
        let dossier = submitted(id, "unused", "unused", 5);
        let json = serde_json::to_string(&dossier).unwrap();
        store.insert_dossier(&id_str, &json).unwrap();

        // A revision produced by arbitration (it has a parent) with an empty residual is
        // converged; resuming such a dossier transitions it without running any agent.
        let converged = Revision {
            revision_id: Uuid::new_v4(),
            parent_digest: Some(Digest("root".into())),
            content_digest: Digest("succ".into()),
            original_proposal: "p".into(),
            current_understanding: "u".into(),
            positions: vec![],
            dissents: vec![],
            risks: vec![],
        };
        store
            .commit_successor_revision(&id_str, None, &converged, &[])
            .unwrap();

        run_deliberation(&id_str, &json, &mut store).unwrap();

        assert_eq!(state_of(&store, &id_str), LifecycleState::ReadyForDecision);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_fresh_empty_dossier_deliberates_before_converging() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("ringi-loop-fresh-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);

        let id = Uuid::new_v4();
        let id_str = id.to_string();

        // The un-deliberated root has an empty residual but must NOT short-circuit to ready.
        // A turn runs; the arbitrator returns an empty successor, which then converges.
        let respondent = fake_agent("resp2.sh", "echo 'nothing to add'");
        let successor_json = format!(
            "{{\"successor_revision\":{{\"revision_id\":\"{rev}\",\"parent_digest\":null,\
             \"content_digest\":\"d\",\"original_proposal\":\"p\",\"current_understanding\":\"deliberated\",\
             \"positions\":[],\"dissents\":[],\"risks\":[]}},\"next_questions\":[]}}",
            rev = Uuid::new_v4(),
        );
        let arbitrator = fake_agent("arb2.sh", &format!("echo '{successor_json}'"));

        let mut store = DossierStore::open(&path).unwrap();
        let dossier = submitted(
            id,
            respondent.to_str().unwrap(),
            arbitrator.to_str().unwrap(),
            1,
        );
        let json = serde_json::to_string(&dossier).unwrap();
        store.insert_dossier(&id_str, &json).unwrap();

        let initial = Revision {
            revision_id: Uuid::new_v4(),
            parent_digest: None,
            content_digest: Digest("init".into()),
            original_proposal: "p".into(),
            current_understanding: "u".into(),
            positions: vec![],
            dissents: vec![],
            risks: vec![],
        };
        store
            .commit_successor_revision(&id_str, None, &initial, &[])
            .unwrap();

        run_deliberation(&id_str, &json, &mut store).unwrap();

        // A turn actually ran (understanding advanced past the root), then it converged.
        let latest = store.get_latest_revision(&id_str).unwrap().unwrap();
        assert_eq!(latest.current_understanding, "deliberated");
        assert_eq!(state_of(&store, &id_str), LifecycleState::ReadyForDecision);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_fixture_turn_parses_single_line_json_and_commits_a_successor() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("ringi-loop-turn-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);

        let id = Uuid::new_v4();
        let id_str = id.to_string();
        let dissent_id = Uuid::new_v4();

        // The arbitrator emits exactly one line of compact JSON (the stopgap contract),
        // carrying the still-unresolved dissent forward, so the dossier does not converge.
        let respondent = fake_agent("resp.sh", "echo 'I propose we proceed.'");
        let successor_json = format!(
            "{{\"successor_revision\":{{\"revision_id\":\"{rev}\",\"parent_digest\":null,\
             \"content_digest\":\"d\",\"original_proposal\":\"p\",\"current_understanding\":\"u2\",\
             \"positions\":[],\"dissents\":[{{\"id\":\"{did}\",\"claim\":\"c\",\"resolved_by\":null}}],\
             \"risks\":[]}},\"next_questions\":[]}}",
            rev = Uuid::new_v4(),
            did = dissent_id,
        );
        let arbitrator = fake_agent("arb.sh", &format!("echo '{successor_json}'"));

        let mut store = DossierStore::open(&path).unwrap();
        let dossier = submitted(
            id,
            respondent.to_str().unwrap(),
            arbitrator.to_str().unwrap(),
            1,
        );
        let json = serde_json::to_string(&dossier).unwrap();
        store.insert_dossier(&id_str, &json).unwrap();

        let initial = Revision {
            revision_id: Uuid::new_v4(),
            parent_digest: None,
            content_digest: Digest("init".into()),
            original_proposal: "p".into(),
            current_understanding: "u".into(),
            positions: vec![],
            dissents: vec![Dissent {
                id: dissent_id,
                claim: "c".into(),
                resolved_by: None,
            }],
            risks: vec![],
        };
        store
            .commit_successor_revision(&id_str, None, &initial, &[])
            .unwrap();

        run_deliberation(&id_str, &json, &mut store).unwrap();

        // The turn ran: a successor revision was committed and its understanding advanced.
        let latest = store.get_latest_revision(&id_str).unwrap().unwrap();
        assert_eq!(latest.current_understanding, "u2");
        // The unresolved dissent persists, so the dossier has not converged.
        assert_eq!(state_of(&store, &id_str), LifecycleState::Deliberating);
        let _ = std::fs::remove_file(&path);
    }
}
