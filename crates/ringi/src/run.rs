//! Run assembly: turn a [`RunConfig`] into the wired production seams and drive the round loop.
//!
//! This is the composition root — the one place that takes a run's configuration and constructs
//! the Build, Review, and Verify seams (`AgentRoundBuilder` + `AgentReviewRunner` over one
//! `SubprocessAdapter`, `CommandVerification`) and drives `run_rounds` over a supplied registry
//! backend. It returns the report; presenting it (the CLI) and persisting it (a durable backend,
//! resume) are separate concerns, deliberately not here.

use std::path::PathBuf;
use std::time::Duration;

use pacta::{Pact, Registry};

use crate::agent::SubprocessAdapter;
use crate::reconcile::{
    AgentReviewRunner, AgentRoundBuilder, Resume, RoundReport, RunJournal, run_rounds_journaled,
};
use crate::verify::{CommandVerification, VerifyCommand};

/// Everything needed to assemble and drive one run. Plain data — the config **file format** and
/// the CLI that populates this are a separate concern (a later change).
#[derive(Debug, Clone)]
pub struct RunConfig {
    /// Stable identity of the run.
    pub run_id: String,
    /// The workspace the agents and verification commands run in.
    pub workspace: PathBuf,
    /// The Agent CLI program to invoke (one CLI backs both Build and Review roles).
    pub agent_program: String,
    /// Arguments passed to the Agent CLI.
    pub agent_args: Vec<String>,
    /// The task the Builder agent should carry out (seeds the Builder prompt).
    pub task: String,
    /// The objective verification commands ringi re-runs itself to certify the goal.
    pub verification: Vec<VerifyCommand>,
    /// Per-invocation wall-clock bound for agent calls.
    pub timeout: Duration,
}

/// Assemble the production seams from `config` and drive the round loop over the backend built by
/// `make`, returning the run's [`RoundReport`]. Backend-agnostic: `make` has the same shape
/// `run_rounds` and the conformance suite use, so the reference in-memory backend and a durable
/// one run the identical assembly. Presentation and persistence are the caller's.
#[must_use]
pub fn run_from_config<R, F>(config: &RunConfig, make: F) -> RoundReport
where
    R: Registry,
    R::Error: std::fmt::Debug,
    F: FnOnce(Vec<Pact>, u64) -> R,
{
    run_from_config_journaled(config, make, &(), None)
}

/// As [`run_from_config`], but with a durable `journal` (recording progress) and an optional
/// `resume` point (continuing an interrupted run). Assembly still neither persists nor presents —
/// it forwards a journal the caller owns to the round loop; the caller's journal does the I/O.
#[must_use]
pub fn run_from_config_journaled<R, F>(
    config: &RunConfig,
    make: F,
    journal: &dyn RunJournal,
    resume: Option<Resume>,
) -> RoundReport
where
    R: Registry,
    R::Error: std::fmt::Debug,
    F: FnOnce(Vec<Pact>, u64) -> R,
{
    // One Agent CLI backs both roles; each seam sets its own role and prompt.
    let adapter = SubprocessAdapter::new(config.agent_program.clone(), config.agent_args.clone());
    let builder = AgentRoundBuilder::new(
        adapter.clone(),
        config.task.clone(),
        config.workspace.clone(),
        config.timeout,
    );
    let reviewer = AgentReviewRunner::new(adapter, config.workspace.clone(), config.timeout);
    let verification =
        CommandVerification::new(config.verification.clone(), config.workspace.clone());
    run_rounds_journaled(
        &config.run_id,
        &builder,
        &reviewer,
        &verification,
        make,
        journal,
        resume,
    )
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::reconcile::StepOutcome;
    use pacta_memory::MemoryRegistry;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    fn script_in(dir: &Path, name: &str, body: &str) -> PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn workspace(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("ringi-run-{}-{tag}", std::process::id()))
    }

    #[test]
    fn a_configured_run_is_assembled_and_converges() {
        let ws = workspace("assembly");
        // One Agent CLI backs both roles, branching on the prompt (read from stdin): the Reviewer
        // prompt says "Review", so it surfaces F1 once then none; anything else is the Builder.
        let agent = script_in(
            &ws,
            "agent.sh",
            "p=$(cat); case \"$p\" in \
             *Review*) if [ -e reviewed ]; then echo '{\"findings\":[]}'; \
             else touch reviewed; echo '{\"findings\":[{\"id\":\"F1\",\"summary\":\"fix\"}]}'; fi;; \
             *) echo '{\"status\":\"built\"}';; esac",
        );
        let green = script_in(&ws, "check.sh", "exit 0");
        let config = RunConfig {
            run_id: "run-assembly-test".to_string(),
            workspace: ws.clone(),
            agent_program: agent.to_string_lossy().to_string(),
            agent_args: Vec::new(),
            task: "do the thing".to_string(),
            verification: vec![VerifyCommand::new(
                green.to_string_lossy().to_string(),
                Vec::new(),
                Duration::from_secs(5),
            )],
            timeout: Duration::from_secs(5),
        };

        let report = run_from_config(&config, MemoryRegistry::seeded);
        assert!(
            report.converged,
            "a configured run must converge: {report:?}"
        );
        assert!(
            report.open_findings.is_empty(),
            "findings must resolve: {report:?}"
        );
    }

    #[test]
    fn the_builder_agent_receives_the_task() {
        let ws = workspace("task");
        // Exits 0 only if the prompt on stdin conveys the configured task -> a converged/succeeded
        // build proves task-awareness (a round-only prompt would exit 1).
        let agent = script_in(
            &ws,
            "needs-task.sh",
            "p=$(cat); case \"$p\" in *'ship the widget'*) exit 0;; *) exit 1;; esac",
        );
        let builder = AgentRoundBuilder::new(
            SubprocessAdapter::new(agent.to_string_lossy().to_string(), Vec::new()),
            "ship the widget",
            ws.clone(),
            Duration::from_secs(5),
        );
        use crate::reconcile::RoundBuilder;
        assert_eq!(
            builder.build(0),
            StepOutcome::Succeeded,
            "the run's task must reach the Builder agent's prompt"
        );
    }
}
