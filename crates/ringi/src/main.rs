//! Ringi: a local automation orchestrator for Agent CLIs.
//!
//! Ringi does not think or edit — Agent CLIs do that. Ringi owns the *ringi process*: it
//! sequences a build-review-verify loop, gates actions behind policy and human approval,
//! verifies objectively, and keeps durable state so a run can resume. The hard mechanics
//! it composes rather than reimplements: durable step lifecycle (pacta), convergence to
//! done (suunta), and exactly-once step idempotency (shaahid). See `PROJECT.md`.
//!
//! This binary is the command surface. `init`, `run`, and `status` are wired; the remaining
//! commands land in later phases (see `BACKLOG.md`) and are still stubbed.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, bail};
use clap::{Parser, Subcommand};

use ringi::config;
use ringi::reconcile::Resume;
use ringi::run::{self, RunConfig};
use ringi::store::{RunRecord, RunState, RunStore, SqliteRegistry};

/// The ringi orchestrator command line.
#[derive(Debug, Parser)]
#[command(name = "ringi", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Create the default configuration and state store.
    Init,
    /// Start a new run against a workspace with a task.
    Run {
        /// Path to the workspace (a Git repository).
        #[arg(long)]
        workspace: String,
        /// The task the Builder agent should carry out.
        #[arg(long)]
        task: String,
    },
    /// Show the status of a run.
    Status { run_id: String },
    /// Inspect a run's rounds, diffs, reviews, and verifications.
    Inspect { run_id: String },
    /// Resume an interrupted run.
    Resume { run_id: String },
    /// Cancel a run.
    Cancel { run_id: String },
    /// List pending approvals.
    Approvals,
    /// Approve a pending action.
    Approve { approval_id: String },
    /// Reject a pending action.
    Reject { approval_id: String },
}

fn main() -> anyhow::Result<ExitCode> {
    match Cli::parse().command {
        Command::Init => init_command().map(|()| ExitCode::SUCCESS),
        Command::Run { workspace, task } => run_command(&workspace, &task),
        Command::Status { run_id } => status_command(&run_id),
        Command::Resume { run_id } => resume_command(&run_id),
        Command::Inspect { .. }
        | Command::Cancel { .. }
        | Command::Approvals
        | Command::Approve { .. }
        | Command::Reject { .. } => {
            bail!("ringi is at project-shape: this command is not implemented yet")
        }
    }
}

/// The one user-scope SQLite store: the Registry's lease state and ringi's domain tables together.
fn store_path() -> PathBuf {
    Path::new(".ringi").join("state.sqlite")
}

/// Ensure the store directory exists and open (provisioning the schema) the durable store.
fn open_store() -> anyhow::Result<RunStore> {
    let path = store_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let store =
        RunStore::open(&path).with_context(|| format!("opening store {}", path.display()))?;
    Ok(store)
}

/// Provision the durable store and scaffold the config, neither destroying existing data.
fn init_command() -> anyhow::Result<()> {
    open_store()?;
    println!("Provisioned the run store at {}.", store_path().display());

    let path = Path::new(config::CONFIG_FILE);
    if path.exists() {
        println!("{} already exists; leaving it unchanged.", path.display());
    } else {
        std::fs::write(path, config::DEFAULT_CONFIG)
            .with_context(|| format!("writing {}", path.display()))?;
        println!(
            "Wrote {}. Edit the [agent] program and [[verification]] commands, then run:\n  ringi run --workspace <path> --task \"<what to do>\"",
            path.display()
        );
    }
    Ok(())
}

/// Load the config, record the run, drive it over the durable store, record the outcome, and
/// present the persisted record. Recording lives here in the driver layer — `run_from_config`
/// itself neither persists nor presents (see the `run-assembly` capability).
fn run_command(workspace: &str, task: &str) -> anyhow::Result<ExitCode> {
    let file = config::load(Path::new(config::CONFIG_FILE))?;
    let config = file.into_run_config(PathBuf::from(workspace), task.to_string());

    let store = open_store()?;
    store
        .create_run(&config.run_id, &config.task, workspace)
        .with_context(|| "recording the run")?;
    // Announce the id on stderr before driving, so an interrupted run's id is discoverable for
    // `ringi resume` (stdout carries the final outcome).
    eprintln!("run {} started", config.run_id);
    drive_and_present(&store, &config, None)
}

/// Resume an interrupted run: reconstruct its config from the recorded task/workspace plus the
/// current config file, load its checkpoint, and drive it forward from there.
fn resume_command(run_id: &str) -> anyhow::Result<ExitCode> {
    let store = open_store()?;
    let Some(resume) = store
        .load_resume(run_id)
        .with_context(|| "loading the run to resume")?
    else {
        bail!("run '{run_id}' is not resumable: unknown id or already finished");
    };
    let record = store
        .get_run(run_id)
        .with_context(|| "reading the run")?
        .expect("load_resume succeeded, so the run exists");

    // The run id is a deterministic function of workspace + task, so rebuilding the config from
    // the recorded workspace/task (plus the current config file) reproduces the same run id.
    let file = config::load(Path::new(config::CONFIG_FILE))?;
    let config = file.into_run_config(PathBuf::from(&record.workspace), record.task);
    drive_and_present(&store, &config, Some(resume))
}

/// Drive a run (fresh or resumed) over the durable store, journalling progress, then record and
/// present the outcome. `run_from_config` itself never persists — the journal here does.
fn drive_and_present(
    store: &RunStore,
    config: &RunConfig,
    resume: Option<Resume>,
) -> anyhow::Result<ExitCode> {
    // The registry backend is the durable store; the `make` closure cannot return a Result, but
    // `open_store` already proved the file is openable, so this is an invariant.
    let db = store_path();
    let report = run::run_from_config_journaled(
        config,
        move |pacts, lease| {
            SqliteRegistry::open_seeded(&db, pacts, lease)
                .expect("durable registry opens over the provisioned store")
        },
        &store.journal(&config.run_id),
        resume,
    );

    store
        .complete_run(
            &config.run_id,
            report.converged,
            report.rounds,
            &report.open_findings,
        )
        .with_context(|| "recording the run outcome")?;

    let record = store
        .get_run(&config.run_id)
        .with_context(|| "reading back the recorded run")?
        .expect("the run was just recorded");
    print!("{}", present(&record));
    Ok(if report.converged {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

/// Read a persisted run and present it; an unknown id fails clearly.
fn status_command(run_id: &str) -> anyhow::Result<ExitCode> {
    let store = open_store()?;
    match store.get_run(run_id).with_context(|| "reading the run")? {
        Some(record) => {
            print!("{}", present(&record));
            Ok(ExitCode::SUCCESS)
        }
        None => bail!(
            "no run with id '{run_id}' in the store at {}",
            store_path().display()
        ),
    }
}

/// Render a persisted run for the terminal: identity, state, rounds, and any open findings.
fn present(record: &RunRecord) -> String {
    let result = match record.state {
        RunState::Running => "running (interrupted or in progress)".to_string(),
        RunState::Converged => format!("converged in {} round(s)", record.rounds),
        RunState::Failed => format!("did not converge within {} round(s)", record.rounds),
    };
    let mut out = String::new();
    out.push_str(&format!("run {}\n", record.run_id));
    out.push_str(&format!("  workspace: {}\n", record.workspace));
    out.push_str(&format!("  task:      {}\n", record.task));
    out.push_str(&format!("  result:    {result}\n"));
    if record.open_findings.is_empty() {
        out.push_str("  findings:  none open\n");
    } else {
        out.push_str(&format!(
            "  findings:  {} open\n",
            record.open_findings.len()
        ));
        for id in &record.open_findings {
            out.push_str(&format!("    - {id}\n"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(state: RunState, rounds: usize, open: &[&str]) -> RunRecord {
        RunRecord {
            run_id: "run-123".to_string(),
            task: "do it".to_string(),
            workspace: "/tmp/ws".to_string(),
            state,
            rounds,
            open_findings: open.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    #[test]
    fn a_converged_run_presents_success_and_no_open_findings() {
        let text = present(&record(RunState::Converged, 2, &[]));
        assert!(text.contains("converged in 2 round(s)"));
        assert!(text.contains("none open"));
        assert!(text.contains("run-123"));
    }

    #[test]
    fn a_failed_run_lists_open_findings() {
        let text = present(&record(RunState::Failed, 8, &["F1", "F2"]));
        assert!(text.contains("did not converge within 8 round(s)"));
        assert!(text.contains("2 open"));
        assert!(text.contains("- F1"));
        assert!(text.contains("- F2"));
    }

    #[test]
    fn a_running_run_presents_as_interrupted_or_in_progress() {
        let text = present(&record(RunState::Running, 0, &[]));
        assert!(text.contains("running"));
    }
}
