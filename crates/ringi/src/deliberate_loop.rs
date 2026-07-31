//! The two Agent-CLI invocation loops: `run_deliberation` (a submitted dossier's turn loop —
//! respondent answers, arbitrator proposes a successor, suunta decides convergence via
//! `convergence::is_ready`) and `evaluate_conditions` (the isolated per-condition evaluator
//! loop for a `ReadyForDecision` dossier). Every invocation of either loop is claimed through
//! `registry.rs`'s `SqliteRegistry` before the agent runs and settled after, via the shared
//! `claimed_invoke` helper — the one place this crate composes pacta for invocation
//! crash-recovery, so neither loop repeats that checkpoint inline.
//!
//! Neither loop authors SSOT content itself: `run_deliberation` applies the arbitrator's
//! declared move batch through `Revision::apply_moves` (via `deliberation::apply_arbitration`),
//! and readiness is never an agent claim.

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

/// What a claimed unit of work decided about its own outcome, distinct from whether it executed
/// without error. Some successful results are still not "final" in the sense that matters to the
/// claim itself — e.g. a condition evaluator's negative verdict is a fully successful invocation
/// that is nonetheless not a permanent fact about the condition.
enum Settlement<T> {
    /// The result is final: the claim never needs to be retried under this coordinate.
    Fulfilled(T),
    /// The result is valid and usable now, but not final: the same coordinate should remain
    /// claimable (e.g. a negative/uncertain verdict whose underlying circumstance might change).
    Retryable(T),
}

impl<T> Settlement<T> {
    fn into_inner(self) -> T {
        match self {
            Settlement::Fulfilled(t) | Settlement::Retryable(t) => t,
        }
    }
}

/// Claims `coordinate` through `registry`, runs `invoke`, and settles the claim according to
/// `invoke`'s own `Settlement` — fulfilled only on `Ok(Settlement::Fulfilled(_))`, released for an
/// immediate retry under the same coordinate on `Ok(Settlement::Retryable(_))` or any `Err`. A
/// claim never settles fulfilled on a bare zero exit code alone — a caller whose success also
/// requires parsing structured output must do that parse *inside* `invoke`, or a malformed
/// response would settle fulfilled (permanently, since fulfilled is terminal) despite ringi never
/// getting a usable result. Centralizes the claim-before-invoke/settle-after checkpoint so the
/// three invocation sites (respondent, arbitrator, condition-evaluator) do not each repeat it.
fn claimed_invoke<T>(
    registry: &SqliteRegistry,
    coordinate: &InvocationCoordinate,
    invoke: impl FnOnce() -> anyhow::Result<Settlement<T>>,
) -> anyhow::Result<T> {
    let ticket = registry.claim_invocation(coordinate)?.with_context(|| {
        format!(
            "cannot claim invocation {} — already settled or held under an unexpired lease",
            coordinate.idempotency_key()
        )
    })?;

    let outcome = invoke();
    match &outcome {
        Ok(Settlement::Fulfilled(_)) => registry.settle_fulfilled(ticket)?,
        Ok(Settlement::Retryable(_)) => registry.release_for_retry(ticket)?,
        Err(_) => registry.release_for_retry(ticket)?,
    }
    outcome.map(Settlement::into_inner)
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

        let respondent_coordinate = InvocationCoordinate {
            dossier_id: Uuid::parse_str(dossier_id).unwrap_or_default(),
            role: "respondent".to_string(),
            input_digest: current_revision.content_digest.clone(),
            turn,
            attempt: 1,
        };

        // A retried turn (arbitrator failed after the respondent already succeeded) reuses the
        // respondent's already-persisted answer instead of re-invoking it — its coordinate is
        // already Settled, so claimed_invoke would refuse to reclaim it anyway.
        let claim = match store.find_event_for_coordinate(dossier_id, &respondent_coordinate)? {
            Some(crate::event::Event {
                payload: crate::event::EventPayload::PublicRecord(claim),
                ..
            }) => {
                println!(
                    "Turn {}: Reusing respondent's already-recorded claim.",
                    turn
                );
                claim
            }
            Some(_) => bail!(
                "respondent coordinate {} has a persisted event with an unexpected payload type",
                respondent_coordinate.idempotency_key()
            ),
            None => {
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
                let claim = claimed_invoke(
                    registry,
                    &respondent_coordinate,
                    || -> anyhow::Result<Settlement<String>> {
                        let res = respondent.run(req)?;
                        if res.exit_code != Some(0) {
                            bail!("Respondent failed: {}", res.stderr);
                        }
                        Ok(Settlement::Fulfilled(res.stdout.trim().to_string()))
                    },
                )?;
                println!("Turn {}: Respondent answered with claim: {}", turn, claim);

                // Persist immediately — not batched with the eventual successor commit — so a
                // later arbitrator failure can never discard a respondent that already succeeded.
                let mut respondent_event = crate::event::Event::new_public(
                    crate::event::EventPayload::PublicRecord(claim.clone()),
                    turn as u64 * 1000,
                );
                respondent_event.coordinate = Some(respondent_coordinate.clone());
                store.record_event(dossier_id, &respondent_event)?;

                claim
            }
        };

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
        // Applying arbitration (apply_moves's per-move validation and atomic batch application)
        // happens *inside* the claim boundary: a response that parses fine but fails that domain
        // validation is just as unusable as a malformed one, and must release the claim for
        // retry, not settle it fulfilled.
        let successor = claimed_invoke(
            registry,
            &arbitrator_coordinate,
            || -> anyhow::Result<Settlement<crate::revision::Revision>> {
                let res = arbitrator.run(arb_agent_req)?;
                if res.exit_code != Some(0) {
                    bail!("Arbitrator failed: {}", res.stderr);
                }
                let metadata = res
                    .metadata
                    .context("Arbitrator produced no structured output")?;
                let output: ArbitrationOutput = serde_json::from_value(metadata)?;
                println!("Turn {}: Applying arbitration...", turn);
                let applied =
                    apply_arbitration(&current_revision, output).map_err(|e| anyhow::anyhow!(e))?;
                Ok(Settlement::Fulfilled(applied))
            },
        )?;

        // The respondent's event is already durably persisted (either just now, or recovered
        // from a prior attempt) — nothing further to commit alongside the successor.
        store.commit_successor_revision(
            dossier_id,
            Some(&current_revision.revision_id.to_string()),
            &successor,
            &[],
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
            || -> anyhow::Result<Settlement<ConditionEvaluationOutput>> {
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
                let output: ConditionEvaluationOutput = serde_json::from_value(metadata)?;
                if output.verdict == ConditionVerdict::True {
                    Ok(Settlement::Fulfilled(output))
                } else {
                    Ok(Settlement::Retryable(output))
                }
            },
        )?;

        let mut evaluation_event = Event::new_sealed(
            EventPayload::SealedEvaluation {
                evaluator: format!("condition:{}", dossier.conditions[index].id),
                reasoning: output.reason.clone(),
            },
            index as u64,
        );

        // Only a True verdict's event carries its coordinate: that settlement is terminal, so
        // recording it under this idempotency key exactly once is correct. A False/Unknown
        // verdict's coordinate stays unset here — the coordinate itself remains claimable (see
        // `claimed_invoke`'s `Settlement::Retryable`), and a later retry under that same
        // coordinate must be able to record its own event without colliding with this one on
        // `idempotency_key` (events, unlike pacts, have no notion of "retry" of their own).
        if output.verdict == ConditionVerdict::True {
            evaluation_event.coordinate = Some(coordinate);
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
            questions: vec![],
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
        // A turn runs; the arbitrator declares an empty move batch, which then converges.
        let respondent = fake_agent("resp2.sh", "echo 'nothing to add'");
        let successor_json = "{\"current_understanding\":\"deliberated\",\"moves\":[]}";
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
            questions: vec![],
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

        // The arbitrator emits exactly one line of compact JSON (the transport contract), with
        // an empty move batch — the still-unresolved dissent carries forward since no move
        // targets it, so the dossier does not converge.
        let respondent = fake_agent("resp.sh", "echo 'I propose we proceed.'");
        let successor_json = "{\"current_understanding\":\"u2\",\"moves\":[]}";
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
            questions: vec![],
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
            questions: vec![],
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
            questions: vec![],
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

    #[test]
    fn a_domain_invalid_arbitrator_response_releases_the_claim_not_fulfills_it() {
        // Structurally valid JSON, but apply_moves rejects it (the move targets a dissent that
        // does not exist): apply_arbitration's validation must be inside the claim boundary too,
        // not just the parse — a response that parses fine but fails domain validation is just
        // as unusable.
        let _guard = crate::PROCESS_CWD_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "ringi-loop-domain-invalid-{}.sqlite",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);

        let id = Uuid::new_v4();
        let id_str = id.to_string();

        let respondent = fake_agent("resp-ok2.sh", "echo 'looks fine'");
        // Valid JSON, but the move targets a dissent id that does not exist on the revision.
        let arbitrator = fake_agent(
            "arb-bad-target.sh",
            r#"echo '{"current_understanding":"u","moves":[{"kind":"ResolveDissent","id":"22222222-2222-2222-2222-222222222222","resolution":{"reason":"r","provenance":[{"event_id":"33333333-3333-3333-3333-333333333333"}]}}]}'"#,
        );

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
            questions: vec![],
        };
        store
            .commit_successor_revision(&id_str, None, &initial, &[])
            .unwrap();

        let err = run_deliberation(&id_str, &json, &mut store, &registry).unwrap_err();
        assert!(
            err.to_string()
                .contains("Move targets a dissent that does not exist")
        );

        // The arbitrator's claim must have been released, not fulfilled, even though its
        // response parsed successfully — domain validation failed after the parse.
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

    #[test]
    fn a_retried_turn_does_not_reinvoke_an_already_succeeded_respondent() {
        let _guard = crate::PROCESS_CWD_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir();
        let path = dir.join(format!("ringi-loop-resume-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let counter_path = dir.join(format!("ringi-loop-resume-count-{}", std::process::id()));
        let _ = std::fs::remove_file(&counter_path);
        let fix_flag_path = dir.join(format!("ringi-loop-resume-fix-{}", std::process::id()));
        let _ = std::fs::remove_file(&fix_flag_path);

        let id = Uuid::new_v4();
        let id_str = id.to_string();

        // Counts invocations by appending one byte per call, so the test can assert the
        // respondent subprocess ran exactly once across both run_deliberation calls.
        let respondent = fake_agent(
            "resp-counting.sh",
            &format!(
                "printf x >> '{}'\necho 'the only answer'",
                counter_path.display()
            ),
        );
        // Fails (malformed output) until fix_flag_path exists, then succeeds — modeling "the
        // underlying problem gets fixed" between the two run_deliberation calls.
        let arbitrator = fake_agent(
            "arb-toggle.sh",
            &format!(
                "if [ -f '{}' ]; then \
                   echo '{{\"current_understanding\":\"u2\",\"moves\":[]}}'; \
                 else \
                   echo 'not json at all'; \
                 fi",
                fix_flag_path.display()
            ),
        );

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
            questions: vec![],
        };
        store
            .commit_successor_revision(&id_str, None, &initial, &[])
            .unwrap();

        // First attempt: respondent succeeds, arbitrator fails on malformed output.
        let err = run_deliberation(&id_str, &json, &mut store, &registry).unwrap_err();
        assert!(err.to_string().contains("no structured output"));
        assert_eq!(
            std::fs::read_to_string(&counter_path).unwrap().len(),
            1,
            "respondent must have been invoked exactly once so far"
        );

        // Fix the underlying problem and retry the same turn.
        std::fs::write(&fix_flag_path, "").unwrap();
        run_deliberation(&id_str, &json, &mut store, &registry)
            .expect("the retried turn should now complete");

        assert_eq!(
            std::fs::read_to_string(&counter_path).unwrap().len(),
            1,
            "the respondent must not be re-invoked on the retried turn"
        );
        assert_eq!(state_of(&store, &id_str), LifecycleState::ReadyForDecision);

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&counter_path);
        let _ = std::fs::remove_file(&fix_flag_path);
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
            questions: vec![],
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
    fn a_false_verdict_releases_the_claim_for_retry() {
        let _guard = crate::PROCESS_CWD_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "ringi-eval-false-retry-{}.sqlite",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);

        let id = Uuid::new_v4();
        let id_str = id.to_string();
        let condition = one_condition();
        let evaluator = fake_agent(
            "eval-false-retry.sh",
            "echo '{\"verdict\":\"False\",\"reason\":\"over budget\",\"provenance\":[]}'",
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

        // A False verdict is a fully successful invocation, but not final: the same coordinate
        // must remain claimable — the exact bug found while dogfooding reopen -> evaluate.
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
            .expect("a False verdict must release the claim for retry, not settle it fulfilled");
        registry.settle_fulfilled(retry_ticket).unwrap();

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn an_unknown_verdict_releases_the_claim_for_retry() {
        let _guard = crate::PROCESS_CWD_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "ringi-eval-unknown-retry-{}.sqlite",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);

        let id = Uuid::new_v4();
        let id_str = id.to_string();
        let condition = one_condition();
        let evaluator = fake_agent(
            "eval-unknown-retry.sh",
            "echo '{\"verdict\":\"Unknown\",\"reason\":\"cannot tell\",\"provenance\":[]}'",
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
            .expect("an Unknown verdict must release the claim for retry, not settle it fulfilled");
        registry.settle_fulfilled(retry_ticket).unwrap();

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_prior_negative_verdict_does_not_block_a_later_evaluate_call_from_reaching_a_subsequent_condition()
     {
        let _guard = crate::PROCESS_CWD_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "ringi-eval-subsequent-{}.sqlite",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);

        let id = Uuid::new_v4();
        let id_str = id.to_string();
        let first = Condition {
            id: Uuid::new_v4(),
            description: "Load test passed".into(),
            is_met: false,
        };
        let second = Condition {
            id: Uuid::new_v4(),
            description: "Legal sign-off received".into(),
            is_met: false,
        };
        // Reads the prompt on stdin and answers based on which condition it is judging, so a
        // single evaluator program can distinguish two conditions in one dossier.
        let evaluator = fake_agent(
            "eval-subsequent.sh",
            "input=$(cat)\n\
             if echo \"$input\" | grep -q 'Load test passed'; then\n\
               echo '{\"verdict\":\"False\",\"reason\":\"still failing\",\"provenance\":[]}'\n\
             else\n\
               echo '{\"verdict\":\"True\",\"reason\":\"signed off\",\"provenance\":[]}'\n\
             fi",
        );

        let mut store = DossierStore::open(&path).unwrap();
        let registry = test_registry(&path);
        let dossier = ready_for_decision_with_conditions(
            id,
            evaluator.to_str().unwrap(),
            vec![first, second.clone()],
        );
        let json = serde_json::to_string(&dossier).unwrap();
        store.insert_dossier(&id_str, &json).unwrap();
        store
            .commit_successor_revision(&id_str, None, &base_revision(), &[])
            .unwrap();

        evaluate_conditions(&id_str, &json, &mut store, &registry).unwrap();

        // Even though the first condition's verdict was negative, the loop must still reach and
        // evaluate the second, still-unattempted condition in the same call.
        let state_json = store.get_dossier_state(&id_str).unwrap().unwrap();
        let updated: SubmittedDossier = serde_json::from_str(&state_json).unwrap();
        assert!(!updated.conditions[0].is_met);
        assert!(
            updated.conditions[1].is_met,
            "a negative verdict on an earlier condition must not prevent the later condition \
             from being reached and evaluated"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_released_condition_can_be_reevaluated_to_true_on_a_later_call() {
        let _guard = crate::PROCESS_CWD_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "ringi-eval-reevaluate-{}.sqlite",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let fix_flag_path = dir.join(format!("ringi-eval-reevaluate-fix-{}", std::process::id()));
        let _ = std::fs::remove_file(&fix_flag_path);

        let id = Uuid::new_v4();
        let id_str = id.to_string();
        let condition = one_condition();
        // Answers False until fix_flag_path exists, then True — modeling "reopen, fix the
        // underlying circumstance, evaluate again" without a real reopen transition.
        let evaluator = fake_agent(
            "eval-toggle.sh",
            &format!(
                "if [ -f '{}' ]; then \
                   echo '{{\"verdict\":\"True\",\"reason\":\"now under budget\",\"provenance\":[]}}'; \
                 else \
                   echo '{{\"verdict\":\"False\",\"reason\":\"over budget\",\"provenance\":[]}}'; \
                 fi",
                fix_flag_path.display()
            ),
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
        let after_first = store.get_dossier_state(&id_str).unwrap().unwrap();
        let after_first: SubmittedDossier = serde_json::from_str(&after_first).unwrap();
        assert!(!after_first.conditions[0].is_met);

        std::fs::write(&fix_flag_path, "").unwrap();
        // Re-read the (unchanged) dossier JSON, matching how a CLI would re-invoke evaluate on
        // the same still-ReadyForDecision dossier after the underlying circumstance changes.
        evaluate_conditions(&id_str, &json, &mut store, &registry)
            .expect("the released coordinate must be reclaimable, re-invoking the evaluator");

        let after_second = store.get_dossier_state(&id_str).unwrap().unwrap();
        let after_second: SubmittedDossier = serde_json::from_str(&after_second).unwrap();
        assert!(
            after_second.conditions[0].is_met,
            "the condition must be able to reach True on a later evaluate call, once its claim \
             was released instead of permanently settled"
        );

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&fix_flag_path);
    }

    #[test]
    fn two_consecutive_negative_verdicts_do_not_collide_on_idempotency_key() {
        let _guard = crate::PROCESS_CWD_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "ringi-eval-double-negative-{}.sqlite",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);

        let id = Uuid::new_v4();
        let id_str = id.to_string();
        let condition = one_condition();
        let evaluator = fake_agent(
            "eval-still-false.sh",
            "echo '{\"verdict\":\"False\",\"reason\":\"still over budget\",\"provenance\":[]}'",
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

        // Two full evaluate_conditions calls, both returning False for the same unchanged
        // revision: neither's sealed event may collide with the other on idempotency_key, even
        // though both were invoked under the exact same InvocationCoordinate.
        evaluate_conditions(&id_str, &json, &mut store, &registry).unwrap();
        evaluate_conditions(&id_str, &json, &mut store, &registry)
            .expect("a second retry under the same coordinate must not fail to record its event");

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
