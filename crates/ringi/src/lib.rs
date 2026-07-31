//! Ringi library.
//!
//! dossier-based execution tracking.
//! See `PROJECT.md` and `BACKLOG.md`.

pub mod agent;
pub mod archive;
pub mod convergence;
pub mod deliberate_loop;
pub mod deliberation;
pub mod dossier;
pub mod dossier_cli;
pub mod event;
pub mod exec;
pub mod registry;
pub mod revision;
pub mod store;

/// The process's current directory is global state shared by every test in this binary, not just
/// the ones that call `std::env::set_current_dir` directly: any test that spawns an agent reads
/// it too (`AgentRequest::working_dir`), so it can be pulled out from under a concurrently-running
/// spawn by an unrelated test's CWD mutation and deletion. Any test that mutates the CWD, or that
/// spawns an agent (reading it transitively), MUST hold this lock for the duration.
#[cfg(test)]
pub(crate) static PROCESS_CWD_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
