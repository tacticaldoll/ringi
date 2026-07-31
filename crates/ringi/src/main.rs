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

use anyhow::Context;
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

    // Dossier commands
    /// Create a new dossier draft.
    Draft,
    /// Submit a dossier draft for deliberation.
    Submit { id: String },
    /// Run synchronous deliberation on a submitted dossier.
    Continue { id: String },
    /// Inspect a dossier.
    Inspect { id: String },
    /// Make a human decision to approve.
    Approve { id: String },
    /// Reject a dossier.
    Reject { id: String },
    /// Cancel a dossier.
    Cancel { id: String },
    /// Invalidate a dossier.
    Invalidate { id: String },
    /// Add a condition to an approved-with-conditions dossier.
    Condition { id: String, description: String },
}

fn main() -> anyhow::Result<std::process::ExitCode> {
    match Cli::parse().command {
        Command::Init => init_command().map(|()| std::process::ExitCode::SUCCESS),
        Command::Draft => {
            ringi::dossier_cli::draft_command().map(|()| std::process::ExitCode::SUCCESS)
        }
        Command::Submit { id } => {
            let mut store = open_dossier_store()?;
            ringi::dossier_cli::submit_command(&id, &mut store)
                .map(|()| std::process::ExitCode::SUCCESS)
        }
        Command::Continue { id } => {
            let mut store = open_dossier_store()?;
            ringi::dossier_cli::continue_command(&id, &mut store)
                .map(|()| std::process::ExitCode::SUCCESS)
        }
        Command::Inspect { id } => {
            let store = open_dossier_store()?;
            ringi::dossier_cli::inspect_command(&id, &store)
                .map(|()| std::process::ExitCode::SUCCESS)
        }
        Command::Approve { id } => {
            let mut store = open_dossier_store()?;
            ringi::dossier_cli::transition_command(
                &id,
                ringi::dossier::LifecycleState::Approved,
                &mut store,
            )
            .map(|()| std::process::ExitCode::SUCCESS)
        }
        Command::Reject { id } => {
            let mut store = open_dossier_store()?;
            ringi::dossier_cli::transition_command(
                &id,
                ringi::dossier::LifecycleState::Rejected,
                &mut store,
            )
            .map(|()| std::process::ExitCode::SUCCESS)
        }
        Command::Cancel { id } => {
            let mut store = open_dossier_store()?;
            ringi::dossier_cli::transition_command(
                &id,
                ringi::dossier::LifecycleState::Cancelled,
                &mut store,
            )
            .map(|()| std::process::ExitCode::SUCCESS)
        }
        Command::Invalidate { id } => {
            let mut store = open_dossier_store()?;
            ringi::dossier_cli::transition_command(
                &id,
                ringi::dossier::LifecycleState::Invalidated,
                &mut store,
            )
            .map(|()| std::process::ExitCode::SUCCESS)
        }
        Command::Condition { id, description } => {
            let mut store = open_dossier_store()?;
            ringi::dossier_cli::add_condition_command(&id, &description, &mut store)
                .map(|()| std::process::ExitCode::SUCCESS)
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
