//! The two Agent-CLI invocation loops: `run_deliberation` (a submitted dossier's turn loop —
//! respondent answers, arbitrator proposes a successor, suunta decides convergence via
//! `convergence::is_ready`) and `evaluate_conditions` (the isolated per-condition evaluator
//! loop for a `ReadyForDecision` dossier). Every invocation of either loop is claimed through
//! `registry.rs`'s `SqliteRegistry` before the agent runs and settled after, via the shared
//! `claimed_invoke` helper — the one place this crate composes pacta for invocation
//! crash-recovery, so neither loop repeats that checkpoint inline.
//!
//! Neither loop authors SSOT content itself: `run_deliberation` applies the arbitrator's
//! proposed successor through `Revision::propose_successor` (via `deliberation::
//! apply_arbitration`), and readiness is never an agent claim.

use anyhow::{Context, bail};
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

use crate::agent::{AgentAdapter, AgentRequest, AgentRole, SubprocessAdapter};
use crate::deliberation::{
    ArbitrationOutput, ConditionEvaluationOutput, ConditionVerdict, apply_arbitration,
    build_arbitrator_prompt, build_condition_evaluator_prompt, build_respondent_prompt,
};
use crate::dossier::{LifecycleState, SubmittedDossier};
use crate::event::{Event, EventPayload, InvocationCoordinate};
use crate::registry::SqliteRegistry;
use crate::store::DossierStore;

/// Claims `coordinate` through `registry`, runs `invoke`, and settles the claim — fulfilled only
/// if `invoke` returns `Ok` (a usable result — the caller decides what "usable" means: an
/// exit-code check, and for the arbitrator/condition-evaluator, a parsed structured output),
/// released for an immediate retry under the same coordinate on any `Err`. A claim never settles
/// fulfilled on a bare zero exit code alone — a caller whose success also requires parsing
/// structured output must do that parse *inside* `invoke`, or a malformed response would settle
/// fulfilled (permanently, since fulfilled is terminal) despite ringi never getting a usable
/// result. Centralizes the claim-before-invoke/settle-after checkpoint so the three invocation
/// sites (respondent, arbitrator, condition-evaluator) do not each repeat it.
fn claimed_invoke<T>(
    registry: &SqliteRegistry,
    coordinate: &InvocationCoordinate,
    invoke: impl FnOnce() -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    let ticket = registry.claim_invocation(coordinate)?.with_context(|| {
        format!(
            "cannot claim invocation {} — already settled or held under an unexpired lease",
            coordinate.idempotency_key()
        )
    })?;

    let outcome = invoke();
    match &outcome {
        Ok(_) => registry.settle_fulfilled(ticket)?,
        Err(_) => registry.release_for_retry(ticket)?,
    }
    outcome
}

pub fn run_deliberation(
    dossier_id: &str,
    dossier_json: &str,
    store: &mut DossierStore,
    registry: &SqliteRegistry,
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
    // convergence still transitions. Here, before any turn, this covers resuming an
    // already-converged dossier — see `is_ready_for_decision`'s doc for why the un-deliberated
    // root is never treated as ready on its own.
    if crate::convergence::is_ready_for_decision(&current_revision) {
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

        let respondent_coordinate = InvocationCoordinate {
            dossier_id: Uuid::parse_str(dossier_id).unwrap_or_default(),
            role: "respondent".to_string(),
            input_digest: current_revision.content_digest.clone(),
            turn,
            attempt: 1,
        };
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
        let claim = claimed_invoke(
            registry,
            &respondent_coordinate,
            || -> anyhow::Result<String> {
                let res = respondent.run(req)?;
                if res.exit_code != Some(0) {
                    bail!("Respondent failed: {}", res.stderr);
                }
                Ok(res.stdout.trim().to_string())
            },
        )?;
        println!("Turn {}: Respondent answered with claim: {}", turn, claim);

        // Record respondent event
        let mut respondent_event = crate::event::Event::new_public(
            crate::event::EventPayload::PublicRecord(claim.clone()),
            turn as u64 * 1000,
        );
        respondent_event.coordinate = Some(respondent_coordinate);

        println!("Turn {}: Building arbitrator prompt...", turn);
        let arbitrator_prompt = build_arbitrator_prompt(&current_revision, &[claim]);

        let arbitrator_coordinate = InvocationCoordinate {
            dossier_id: Uuid::parse_str(dossier_id).unwrap_or_default(),
            role: "arbitrator".to_string(),
            input_digest: current_revision.content_digest.clone(),
            turn,
            attempt: 1,
        };
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
        let resolution_output = claimed_invoke(
            registry,
            &arbitrator_coordinate,
            || -> anyhow::Result<ArbitrationOutput> {
                let res = arbitrator.run(arb_agent_req)?;
                if res.exit_code != Some(0) {
                    bail!("Arbitrator failed: {}", res.stderr);
                }
                let metadata = res
                    .metadata
                    .context("Arbitrator produced no structured output")?;
                Ok(serde_json::from_value(metadata)?)
            },
        )?;

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

/// Judges every unmet condition on a `ReadyForDecision` dossier with an isolated
/// `ConditionEvaluator` invocation. A `True` verdict marks the condition met; `False` and
/// `Unknown` leave it unmet — conservative, matching the Unknown-is-never-success principle
/// `convergence` already applies to dissents and risks. Each verdict's reasoning is sealed.
pub fn evaluate_conditions(
    dossier_id: &str,
    dossier_json: &str,
    store: &mut DossierStore,
    registry: &SqliteRegistry,
) -> anyhow::Result<()> {
    let mut dossier: SubmittedDossier = serde_json::from_str(dossier_json)?;

    if dossier.state != LifecycleState::ReadyForDecision {
        bail!(
            "Dossier {} is not ReadyForDecision (state: {:?})",
            dossier_id,
            dossier.state
        );
    }

    let revision = store
        .get_latest_revision(dossier_id)?
        .context("No revisions found - cannot evaluate conditions")?;
    let evaluator_program = dossier.locked_settings.roles.respondent.clone();

    for index in 0..dossier.conditions.len() {
        if dossier.conditions[index].is_met {
            continue;
        }

        let prompt = build_condition_evaluator_prompt(&dossier.conditions[index], &revision);
        let coordinate = InvocationCoordinate {
            dossier_id: Uuid::parse_str(dossier_id).unwrap_or_default(),
            role: format!("condition_evaluator:{}", dossier.conditions[index].id),
            input_digest: revision.content_digest.clone(),
            turn: index as u32,
            attempt: 1,
        };
        let evaluator = SubprocessAdapter::new(evaluator_program.clone(), vec![]);
        let req = AgentRequest {
            role: AgentRole::ConditionEvaluator,
            session_instruction: None,
            prompt,
            working_dir: std::env::current_dir()?,
            timeout: Duration::from_secs(60),
            env: HashMap::new(),
        };

        let condition_id = dossier.conditions[index].id;
        let output = claimed_invoke(
            registry,
            &coordinate,
            || -> anyhow::Result<ConditionEvaluationOutput> {
                let res = evaluator.run(req)?;
                if res.exit_code != Some(0) {
                    bail!(
                        "Condition evaluator failed for condition {}: {}",
                        condition_id,
                        res.stderr
                    );
                }
                let metadata = res
                    .metadata
                    .context("Condition evaluator produced no structured output")?;
                Ok(serde_json::from_value(metadata)?)
            },
        )?;

        let mut evaluation_event = Event::new_sealed(
            EventPayload::SealedEvaluation {
                evaluator: format!("condition:{}", dossier.conditions[index].id),
                reasoning: output.reason.clone(),
            },
            index as u64,
        );
        evaluation_event.coordinate = Some(coordinate);

        if output.verdict == ConditionVerdict::True {
            dossier.conditions[index].is_met = true;
        }

        let updated_json = serde_json::to_string(&dossier)?;
        store.record_condition_evaluation(dossier_id, &updated_json, &evaluation_event)?;
    }

    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::dossier::{
        ArbitrationSettings, Condition, Limits, LockedSettings, RoleBindings, StrategyPreset,
        SubmittedDossier,
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

    /// A registry over the same file `path` the test's `DossierStore` uses — two connections to
    /// one file, matching production.
    fn test_registry(path: &std::path::Path) -> SqliteRegistry {
        SqliteRegistry::open(path).unwrap()
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

    fn ready_for_decision_with_conditions(
        id: Uuid,
        respondent: &str,
        conditions: Vec<Condition>,
    ) -> SubmittedDossier {
        SubmittedDossier {
            id,
            state: LifecycleState::ReadyForDecision,
            locked_settings: LockedSettings {
                arbitration: ArbitrationSettings::resolve(StrategyPreset::Economy),
                limits: Limits { max_turns: 1 },
                roles: RoleBindings {
                    respondent: respondent.to_string(),
                    arbitrator: "unused".to_string(),
                },
            },
            conditions,
        }
    }

    fn one_condition() -> Condition {
        Condition {
            id: Uuid::new_v4(),
            description: "Budget is under $1000".into(),
            is_met: false,
        }
    }

    #[test]
    fn a_deliberated_converged_revision_transitions_on_resume() {
        let _guard = crate::PROCESS_CWD_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir();
        let path = dir.join(format!("ringi-loop-resume-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);

        let id = Uuid::new_v4();
        let id_str = id.to_string();
        let mut store = DossierStore::open(&path).unwrap();
        let registry = test_registry(&path);
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

        run_deliberation(&id_str, &json, &mut store, &registry).unwrap();

        assert_eq!(state_of(&store, &id_str), LifecycleState::ReadyForDecision);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_fresh_empty_dossier_deliberates_before_converging() {
        let _guard = crate::PROCESS_CWD_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
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
        let registry = test_registry(&path);
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

        run_deliberation(&id_str, &json, &mut store, &registry).unwrap();

        // A turn actually ran (understanding advanced past the root), then it converged.
        let latest = store.get_latest_revision(&id_str).unwrap().unwrap();
        assert_eq!(latest.current_understanding, "deliberated");
        assert_eq!(state_of(&store, &id_str), LifecycleState::ReadyForDecision);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_fixture_turn_parses_single_line_json_and_commits_a_successor() {
        let _guard = crate::PROCESS_CWD_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
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
        let registry = test_registry(&path);
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

        run_deliberation(&id_str, &json, &mut store, &registry).unwrap();

        // The turn ran: a successor revision was committed and its understanding advanced.
        let latest = store.get_latest_revision(&id_str).unwrap().unwrap();
        assert_eq!(latest.current_understanding, "u2");
        // The unresolved dissent persists, so the dossier has not converged.
        assert_eq!(state_of(&store, &id_str), LifecycleState::Deliberating);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn an_already_settled_coordinate_is_not_reinvoked() {
        let _guard = crate::PROCESS_CWD_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir();
        let path = dir.join(format!("ringi-loop-settled-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);

        let id = Uuid::new_v4();
        let id_str = id.to_string();
        let marker = dir.join(format!("ringi-loop-settled-marker-{}", std::process::id()));
        let _ = std::fs::remove_file(&marker);
        // If the respondent is ever actually invoked, it leaves a marker file behind.
        let respondent = fake_agent(
            "resp-should-not-run.sh",
            &format!("touch {}\necho 'should not run'", marker.display()),
        );

        let mut store = DossierStore::open(&path).unwrap();
        let registry = test_registry(&path);
        let dossier = submitted(id, respondent.to_str().unwrap(), "unused", 1);
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

        // Pre-claim and fulfill exactly the coordinate turn 1's respondent invocation would use,
        // simulating "this attempt already ran and was recorded as settled" without a matching
        // event/revision commit having happened (the crash-window this change protects).
        let coordinate = InvocationCoordinate {
            dossier_id: id,
            role: "respondent".to_string(),
            input_digest: initial.content_digest.clone(),
            turn: 1,
            attempt: 1,
        };
        let ticket = registry
            .claim_invocation(&coordinate)
            .unwrap()
            .expect("a fresh coordinate should be claimable");
        registry.settle_fulfilled(ticket).unwrap();

        let err = run_deliberation(&id_str, &json, &mut store, &registry).unwrap_err();
        assert!(err.to_string().contains("cannot claim invocation"));
        assert!(
            !marker.exists(),
            "the respondent must never be invoked for an already-settled coordinate"
        );

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&marker);
    }

    #[test]
    fn a_malformed_arbitrator_response_releases_the_claim_not_fulfills_it() {
        let _guard = crate::PROCESS_CWD_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "ringi-loop-malformed-{}.sqlite",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);

        let id = Uuid::new_v4();
        let id_str = id.to_string();

        let respondent = fake_agent("resp-ok.sh", "echo 'looks fine'");
        // Exits zero, but is not valid JSON — the exact shape of bug this test guards against.
        let arbitrator = fake_agent("arb-garbled.sh", "echo 'not json at all'");

        let mut store = DossierStore::open(&path).unwrap();
        let registry = test_registry(&path);
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

        let err = run_deliberation(&id_str, &json, &mut store, &registry).unwrap_err();
        assert!(err.to_string().contains("no structured output"));

        // The arbitrator's claim must have been released, not fulfilled: the exact same
        // coordinate is claimable again, proving it did not settle terminally on the bad output.
        let arbitrator_coordinate = InvocationCoordinate {
            dossier_id: id,
            role: "arbitrator".to_string(),
            input_digest: initial.content_digest.clone(),
            turn: 1,
            attempt: 1,
        };
        let retry_ticket = registry
            .claim_invocation(&arbitrator_coordinate)
            .unwrap()
            .expect("a released coordinate must be claimable again, not stuck as fulfilled");
        registry.settle_fulfilled(retry_ticket).unwrap();

        let _ = std::fs::remove_file(&path);
    }

    fn base_revision() -> Revision {
        Revision {
            revision_id: Uuid::new_v4(),
            parent_digest: None,
            content_digest: Digest("dig".into()),
            original_proposal: "p".into(),
            current_understanding: "u".into(),
            positions: vec![],
            dissents: vec![],
            risks: vec![],
        }
    }

    #[test]
    fn a_true_verdict_marks_the_condition_met() {
        let _guard = crate::PROCESS_CWD_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir();
        let path = dir.join(format!("ringi-eval-true-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);

        let id = Uuid::new_v4();
        let id_str = id.to_string();
        let condition = one_condition();
        let evaluator = fake_agent(
            "eval-true.sh",
            "echo '{\"verdict\":\"True\",\"reason\":\"under budget\",\"provenance\":[]}'",
        );

        let mut store = DossierStore::open(&path).unwrap();
        let registry = test_registry(&path);
        let dossier = ready_for_decision_with_conditions(
            id,
            evaluator.to_str().unwrap(),
            vec![condition.clone()],
        );
        let json = serde_json::to_string(&dossier).unwrap();
        store.insert_dossier(&id_str, &json).unwrap();
        store
            .commit_successor_revision(&id_str, None, &base_revision(), &[])
            .unwrap();

        evaluate_conditions(&id_str, &json, &mut store, &registry).unwrap();

        let state_json = store.get_dossier_state(&id_str).unwrap().unwrap();
        let updated: SubmittedDossier = serde_json::from_str(&state_json).unwrap();
        assert!(updated.conditions[0].is_met);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_false_verdict_leaves_the_condition_unmet() {
        let _guard = crate::PROCESS_CWD_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir();
        let path = dir.join(format!("ringi-eval-false-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);

        let id = Uuid::new_v4();
        let id_str = id.to_string();
        let evaluator = fake_agent(
            "eval-false.sh",
            "echo '{\"verdict\":\"False\",\"reason\":\"over budget\",\"provenance\":[]}'",
        );

        let mut store = DossierStore::open(&path).unwrap();
        let registry = test_registry(&path);
        let dossier = ready_for_decision_with_conditions(
            id,
            evaluator.to_str().unwrap(),
            vec![one_condition()],
        );
        let json = serde_json::to_string(&dossier).unwrap();
        store.insert_dossier(&id_str, &json).unwrap();
        store
            .commit_successor_revision(&id_str, None, &base_revision(), &[])
            .unwrap();

        evaluate_conditions(&id_str, &json, &mut store, &registry).unwrap();

        let state_json = store.get_dossier_state(&id_str).unwrap().unwrap();
        let updated: SubmittedDossier = serde_json::from_str(&state_json).unwrap();
        assert!(!updated.conditions[0].is_met);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn an_unknown_verdict_leaves_the_condition_unmet() {
        let _guard = crate::PROCESS_CWD_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir();
        let path = dir.join(format!("ringi-eval-unknown-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);

        let id = Uuid::new_v4();
        let id_str = id.to_string();
        let evaluator = fake_agent(
            "eval-unknown.sh",
            "echo '{\"verdict\":\"Unknown\",\"reason\":\"cannot tell\",\"provenance\":[]}'",
        );

        let mut store = DossierStore::open(&path).unwrap();
        let registry = test_registry(&path);
        let dossier = ready_for_decision_with_conditions(
            id,
            evaluator.to_str().unwrap(),
            vec![one_condition()],
        );
        let json = serde_json::to_string(&dossier).unwrap();
        store.insert_dossier(&id_str, &json).unwrap();
        store
            .commit_successor_revision(&id_str, None, &base_revision(), &[])
            .unwrap();

        evaluate_conditions(&id_str, &json, &mut store, &registry).unwrap();

        let state_json = store.get_dossier_state(&id_str).unwrap().unwrap();
        let updated: SubmittedDossier = serde_json::from_str(&state_json).unwrap();
        assert!(!updated.conditions[0].is_met);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn evaluate_conditions_rejects_a_dossier_not_ready_for_decision() {
        let _guard = crate::PROCESS_CWD_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir();
        let path = dir.join(format!("ringi-eval-badstate-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);

        let id = Uuid::new_v4();
        let id_str = id.to_string();
        let dossier = submitted(id, "unused", "unused", 1);
        let json = serde_json::to_string(&dossier).unwrap();

        let mut store = DossierStore::open(&path).unwrap();
        let registry = test_registry(&path);
        store.insert_dossier(&id_str, &json).unwrap();

        let err = evaluate_conditions(&id_str, &json, &mut store, &registry).unwrap_err();
        assert!(err.to_string().contains("not ReadyForDecision"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_malformed_evaluator_response_releases_the_claim_not_fulfills_it() {
        let _guard = crate::PROCESS_CWD_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "ringi-eval-malformed-{}.sqlite",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);

        let id = Uuid::new_v4();
        let id_str = id.to_string();
        let condition = one_condition();
        // Exits zero, but is not valid JSON — the exact shape of bug this test guards against.
        let evaluator = fake_agent("eval-garbled.sh", "echo 'not json at all'");

        let mut store = DossierStore::open(&path).unwrap();
        let registry = test_registry(&path);
        let dossier = ready_for_decision_with_conditions(
            id,
            evaluator.to_str().unwrap(),
            vec![condition.clone()],
        );
        let json = serde_json::to_string(&dossier).unwrap();
        store.insert_dossier(&id_str, &json).unwrap();
        store
            .commit_successor_revision(&id_str, None, &base_revision(), &[])
            .unwrap();

        let err = evaluate_conditions(&id_str, &json, &mut store, &registry).unwrap_err();
        assert!(err.to_string().contains("no structured output"));

        // The evaluator's claim must have been released, not fulfilled: the exact same
        // coordinate is claimable again.
        let coordinate = InvocationCoordinate {
            dossier_id: id,
            role: format!("condition_evaluator:{}", condition.id),
            input_digest: base_revision().content_digest.clone(),
            turn: 0,
            attempt: 1,
        };
        let retry_ticket = registry
            .claim_invocation(&coordinate)
            .unwrap()
            .expect("a released coordinate must be claimable again, not stuck as fulfilled");
        registry.settle_fulfilled(retry_ticket).unwrap();

        let state_json = store.get_dossier_state(&id_str).unwrap().unwrap();
        let updated: SubmittedDossier = serde_json::from_str(&state_json).unwrap();
        assert!(
            !updated.conditions[0].is_met,
            "a malformed evaluation must not mark the condition met"
        );

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn sealed_evaluator_reasoning_never_reaches_a_respondent_prompt() {
        let _guard = crate::PROCESS_CWD_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir();
        let path = dir.join(format!("ringi-eval-sealed-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);

        let id = Uuid::new_v4();
        let id_str = id.to_string();
        let evaluator = fake_agent(
            "eval-sealed.sh",
            "echo '{\"verdict\":\"False\",\"reason\":\"sealed reason: vendor unresponsive\",\"provenance\":[]}'",
        );

        let mut store = DossierStore::open(&path).unwrap();
        let registry = test_registry(&path);
        let dossier = ready_for_decision_with_conditions(
            id,
            evaluator.to_str().unwrap(),
            vec![one_condition()],
        );
        let json = serde_json::to_string(&dossier).unwrap();
        store.insert_dossier(&id_str, &json).unwrap();
        store
            .commit_successor_revision(&id_str, None, &base_revision(), &[])
            .unwrap();

        evaluate_conditions(&id_str, &json, &mut store, &registry).unwrap();

        let latest = store.get_latest_revision(&id_str).unwrap().unwrap();
        let prompt = build_respondent_prompt("Anything to add?", &latest);
        assert!(!prompt.contains("sealed reason: vendor unresponsive"));
        let _ = std::fs::remove_file(&path);
    }
}
