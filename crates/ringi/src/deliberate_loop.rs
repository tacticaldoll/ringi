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

    let settings = &dossier.locked_settings;
    let max_turns = settings.limits.max_turns;
    let mut turn = 1;

    while turn <= max_turns {
        if current_revision.readiness {
            println!("Dossier {} is ready for decision.", dossier_id);
            dossier
                .transition_to(LifecycleState::ReadyForDecision)
                .map_err(|e| anyhow::anyhow!(e))?;
            store.insert_dossier(dossier_id, &serde_json::to_string(&dossier)?)?;
            break;
        }

        println!("--- Turn {} ---", turn);

        let question = "Please review the unresolved dissents and risks and provide a claim on how to proceed.".to_string();
        let respondent_prompt = build_respondent_prompt(&question, &current_revision);

        let respondent = SubprocessAdapter::new(settings.roles.respondent.clone(), vec![]);
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

        let arbitrator = SubprocessAdapter::new(settings.roles.arbitrator.clone(), vec![]);

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

        let (mut successor, _next_questions, _readiness) =
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

        // Move to the next turn
        current_revision = successor;
        turn += 1;
    }

    if turn > max_turns && !current_revision.readiness {
        println!(
            "Dossier {} reached max turns ({}) without readiness.",
            dossier_id, max_turns
        );
    }

    Ok(())
}
