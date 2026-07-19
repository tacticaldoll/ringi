//! Ringi: a local automation orchestrator for Agent CLIs.
//!
//! Ringi does not think or edit — Agent CLIs do that. Ringi owns the *ringi process*: it
//! sequences a build-review-verify loop, gates actions behind policy and human approval,
//! verifies objectively, and keeps durable state so a run can resume. The hard mechanics
//! it composes rather than reimplements: durable step lifecycle (pacta), convergence to
//! done (suunta), and exactly-once step idempotency (shaahid). See `PROJECT.md`.
//!
//! This is the shape skeleton: the command surface exists; behavior lands increment by
//! increment, bet-first (see `BACKLOG.md`).

use clap::{Parser, Subcommand};

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

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Init
        | Command::Run { .. }
        | Command::Status { .. }
        | Command::Inspect { .. }
        | Command::Resume { .. }
        | Command::Cancel { .. }
        | Command::Approvals
        | Command::Approve { .. }
        | Command::Reject { .. } => {
            anyhow::bail!("ringi is at project-shape: this command is not implemented yet")
        }
    }
}
