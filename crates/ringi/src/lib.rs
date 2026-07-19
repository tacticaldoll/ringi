//! Ringi library.
//!
//! The orchestrator emerges from composition: [`reconcile`] wires suunta (planning and
//! convergence), shaahid (exactly-once), and pacta (durable step lifecycle) into one loop.
//! See `PROJECT.md` and `BACKLOG.md`.

pub mod agent;
pub mod config;
pub mod exec;
pub mod reconcile;
pub mod run;
pub mod store;
pub mod verify;
