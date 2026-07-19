//! Ringi: a local automation orchestrator for Agent CLIs.
//!
//! Ringi does not think or edit — Agent CLIs do that. Ringi owns the *ringi process*: it
//! sequences a build-review-verify loop, gates actions behind policy and human approval,
//! verifies objectively, and keeps durable state so a run can resume. The hard mechanics
//! it composes rather than reimplements: durable step lifecycle (pacta), convergence to
//! done (suunta), and exactly-once step idempotency (shaahid). See `PROJECT.md`.
//!
//! This binary is the command surface. `run` and `init` are wired; the remaining commands
//! land in later phases (see `BACKLOG.md`) and are still stubbed.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, bail};
use clap::{Parser, Subcommand};
use pacta_memory::MemoryRegistry;

use ringi::config;
use ringi::reconcile::RoundReport;
use ringi::run::{self, RunConfig};

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
        Command::Status { .. }
        | Command::Inspect { .. }
        | Command::Resume { .. }
        | Command::Cancel { .. }
        | Command::Approvals
        | Command::Approve { .. }
        | Command::Reject { .. } => {
            bail!("ringi is at project-shape: this command is not implemented yet")
        }
    }
}

/// Write the default config, refusing to clobber an existing one.
fn init_command() -> anyhow::Result<()> {
    let path = Path::new(config::CONFIG_FILE);
    if path.exists() {
        println!("{} already exists; leaving it unchanged.", path.display());
        return Ok(());
    }
    std::fs::write(path, config::DEFAULT_CONFIG)
        .with_context(|| format!("writing {}", path.display()))?;
    println!(
        "Wrote {}. Edit the [agent] program and [[verification]] commands, then run:\n  ringi run --workspace <path> --task \"<what to do>\"",
        path.display()
    );
    Ok(())
}

/// Load the config, drive the run over the in-memory backend, present the outcome, and map
/// convergence to the exit status. The registry backend is in-memory for now; because
/// [`run::run_from_config`] is backend-agnostic, a durable backend swaps in here in a later phase.
fn run_command(workspace: &str, task: &str) -> anyhow::Result<ExitCode> {
    let file = config::load(Path::new(config::CONFIG_FILE))?;
    let config = file.into_run_config(PathBuf::from(workspace), task.to_string());
    let report = run::run_from_config(&config, MemoryRegistry::seeded);
    print!("{}", present(&config, &report));
    Ok(if report.converged {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

/// Render a run's outcome for the terminal: identity, convergence, rounds, and any open findings.
fn present(config: &RunConfig, report: &RoundReport) -> String {
    let mut out = String::new();
    out.push_str(&format!("run {}\n", config.run_id));
    out.push_str(&format!("  workspace: {}\n", config.workspace.display()));
    out.push_str(&format!("  task:      {}\n", config.task));
    if report.converged {
        out.push_str(&format!(
            "  result:    converged in {} round(s)\n",
            report.rounds
        ));
    } else {
        out.push_str(&format!(
            "  result:    did not converge within {} round(s)\n",
            report.rounds
        ));
    }
    if report.open_findings.is_empty() {
        out.push_str("  findings:  none open\n");
    } else {
        out.push_str(&format!(
            "  findings:  {} open\n",
            report.open_findings.len()
        ));
        for id in &report.open_findings {
            out.push_str(&format!("    - {id}\n"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    fn report(converged: bool, rounds: usize, open: &[&str]) -> RoundReport {
        RoundReport {
            rounds,
            converged,
            build_executions: HashMap::new(),
            reclaimed: HashSet::new(),
            bearing_sizes: Vec::new(),
            open_findings: open.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    fn config() -> RunConfig {
        RunConfig {
            run_id: "run-123".to_string(),
            workspace: PathBuf::from("/tmp/ws"),
            agent_program: "agent".to_string(),
            agent_args: Vec::new(),
            task: "do it".to_string(),
            verification: Vec::new(),
            timeout: std::time::Duration::from_secs(5),
        }
    }

    #[test]
    fn a_converged_run_presents_success_and_no_open_findings() {
        let text = present(&config(), &report(true, 2, &[]));
        assert!(text.contains("converged in 2 round(s)"));
        assert!(text.contains("none open"));
        assert!(text.contains("run-123"));
    }

    #[test]
    fn a_non_converged_run_lists_open_findings() {
        let text = present(&config(), &report(false, 8, &["F1", "F2"]));
        assert!(text.contains("did not converge within 8 round(s)"));
        assert!(text.contains("2 open"));
        assert!(text.contains("- F1"));
        assert!(text.contains("- F2"));
    }
}
