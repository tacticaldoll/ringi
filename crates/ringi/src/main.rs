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

    /// Inspect a run's rounds, diffs, reviews, and verifications.
    Inspect { run_id: String },
    /// Cancel a run.
    Cancel { run_id: String },
    /// List pending approvals.
    Approvals,
    /// Approve a pending action.
    Approve { approval_id: String },
    /// Reject a pending action.
    Reject { approval_id: String },

    // Dossier commands
    /// Create a new dossier draft.
    Draft,
    /// Submit a dossier draft for deliberation.
    Submit { id: String },
    /// Run synchronous deliberation on a submitted dossier.
    Deliberate { id: String },
    /// Make a human decision on a ready dossier.
    Decide {
        id: String,
        #[arg(long, group = "decision")]
        approve: bool,
        #[arg(long, group = "decision")]
        reject: bool,
    },
}

fn main() -> anyhow::Result<ExitCode> {
    match Cli::parse().command {
        Command::Init => init_command().map(|()| ExitCode::SUCCESS),
        Command::Draft => ringi::dossier_cli::draft_command().map(|()| ExitCode::SUCCESS),
        Command::Submit { id } => {
            let mut store = open_dossier_store()?;
            ringi::dossier_cli::submit_command(&id, &mut store).map(|()| ExitCode::SUCCESS)
        }
        Command::Deliberate { id } => {
            let mut store = open_dossier_store()?;
            ringi::dossier_cli::deliberate_command(&id, &mut store).map(|()| ExitCode::SUCCESS)
        }
        Command::Decide {
            id,
            approve,
            reject,
        } => {
            let store = open_dossier_store()?;
            ringi::dossier_cli::decide_command(&id, approve, reject, &store)
                .map(|()| ExitCode::SUCCESS)
        }
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

fn open_dossier_store() -> anyhow::Result<ringi::store::DossierStore> {
    let path = store_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let store = ringi::store::DossierStore::open(&path)
        .with_context(|| format!("opening dossier store {}", path.display()))?;
    Ok(store)
}

/// Provision the durable store and scaffold the config, neither destroying existing data.
fn init_command() -> anyhow::Result<()> {
    open_dossier_store()?;
    Ok(())
}
