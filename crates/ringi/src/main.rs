//! Ringi: a local deliberation application for Agent CLIs.
//!
//! Ringi does not think or edit — Agent CLIs do that. A dossier moves through
//! draft → submit → answer → arbitrate → decide → archive: respondents answer bounded
//! questions, an independent arbitrator maintains the durable revision, and a human records the
//! final decision. The hard mechanics it composes rather than reimplements: durable
//! claim/settle around each Agent-CLI invocation (pacta, via `registry`), and mechanical
//! convergence over the residual (suunta, via `convergence`). Exactly-once invocation
//! idempotency (shaahid) is not yet attached — see `BACKLOG.md`'s Family Dependency Stance.
//! See `PROJECT.md`.
//!
//! This binary is the dossier command surface.

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
    /// Add a condition to a dossier in ReadyForDecision.
    Condition { id: String, description: String },
    /// Judge a dossier's unmet conditions with isolated evaluator invocations.
    Evaluate { id: String },
    /// Reopen an ApprovedWithConditions dossier back to ReadyForDecision so `evaluate` can run.
    Reopen { id: String },
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
            let registry = open_registry()?;
            ringi::dossier_cli::continue_command(&id, &mut store, &registry)
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
        Command::Evaluate { id } => {
            let mut store = open_dossier_store()?;
            let registry = open_registry()?;
            ringi::dossier_cli::evaluate_command(&id, &mut store, &registry)
                .map(|()| std::process::ExitCode::SUCCESS)
        }
        Command::Reopen { id } => {
            let mut store = open_dossier_store()?;
            ringi::dossier_cli::transition_command(
                &id,
                ringi::dossier::LifecycleState::ReadyForDecision,
                &mut store,
            )
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

/// Opens the pacta registry over the same file `open_dossier_store` uses — two connections to
/// one file, each with its own busy timeout.
fn open_registry() -> anyhow::Result<ringi::registry::SqliteRegistry> {
    let path = store_path();
    ringi::registry::SqliteRegistry::open(&path)
        .with_context(|| format!("opening registry {}", path.display()))
}

/// Provision the durable store and scaffold the config, neither destroying existing data.
fn init_command() -> anyhow::Result<()> {
    open_dossier_store()?;
    Ok(())
}
