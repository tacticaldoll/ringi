//! The hardened subprocess primitive: spawn a program with arguments (never a shell),
//! bounded by a timeout, with concurrent pipe drain.
//!
//! This is ringi's own blood — one place to get non-shell spawning, environment
//! minimization, and timeout-kill right. Both the agent seam ([`crate::agent`]) and the
//! verification runner ([`crate::verify`]) compose it rather than each hand-rolling a spawn
//! path. The transport does not judge outcomes: it reports the exit code and captured
//! output; whether that is acceptable is the caller's decision.
//!
//! Synchronous by design (v1 runs one run at a time); async is deferred until concurrency
//! forces it.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use wait_timeout::ChildExt;

/// The captured result of a finished subprocess. The process *ran*; whether its exit status
/// is acceptable is the caller's judgment.
#[derive(Debug, Clone)]
pub struct Output {
    /// The process exit code, if the process exited normally.
    pub exit_code: Option<i32>,
    /// Captured standard output.
    pub stdout: String,
    /// Captured standard error.
    pub stderr: String,
}

/// An infrastructure failure running a subprocess — distinct from the process exiting
/// non-zero (which is reported in [`Output`], not raised).
#[derive(Debug)]
pub enum ExecError {
    /// The process could not be spawned.
    Spawn(std::io::Error),
    /// The process did not exit within its timeout and was terminated.
    TimedOut,
    /// An I/O failure while communicating with the process.
    Io(std::io::Error),
}

impl std::fmt::Display for ExecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spawn(error) => write!(f, "failed to spawn process: {error}"),
            Self::TimedOut => write!(f, "process did not finish within its timeout"),
            Self::Io(error) => write!(f, "process I/O error: {error}"),
        }
    }
}

impl std::error::Error for ExecError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Spawn(error) | Self::Io(error) => Some(error),
            Self::TimedOut => None,
        }
    }
}

/// A minimized base environment: the parent's `PATH` (so programs resolve) and a stable
/// locale — nothing else, so ambient secrets do not leak by default. Full redaction is the
/// isolation phase.
fn minimal_base_env() -> Vec<(String, String)> {
    let mut env = vec![("LANG".to_string(), "C".to_string())];
    if let Ok(path) = std::env::var("PATH") {
        env.push(("PATH".to_string(), path));
    }
    env
}

/// Run `program` with `args` in `workspace`, feeding `stdin` and then closing it (so a reader
/// sees EOF), bounded by `timeout`. Spawned as program + arguments, **never through a shell**,
/// over a minimized base environment extended by `env`.
///
/// Returns the captured [`Output`] on a normal exit (any code), or an [`ExecError`] for an
/// infrastructure failure (spawn failure, timeout, or I/O error).
pub fn run(
    program: &str,
    args: &[String],
    workspace: &Path,
    env: &HashMap<String, String>,
    stdin: &str,
    timeout: Duration,
) -> Result<Output, ExecError> {
    // program + args, never a shell.
    let mut child = Command::new(program)
        .args(args)
        .current_dir(workspace)
        .env_clear()
        .envs(minimal_base_env())
        .envs(env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(ExecError::Spawn)?;

    // Service all three pipes on their own threads so none can fill and deadlock the child:
    // feeding stdin, and draining stdout/stderr, all proceed concurrently while the main
    // thread waits on the timeout.
    let mut stdin_pipe = child.stdin.take().expect("stdin piped");
    let input = stdin.to_string();
    std::thread::spawn(move || {
        let _ = stdin_pipe.write_all(input.as_bytes());
        // stdin_pipe dropped here -> closed, so a process reading stdin sees EOF.
    });
    let mut out_pipe = child.stdout.take().expect("stdout piped");
    let mut err_pipe = child.stderr.take().expect("stderr piped");
    let out_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = out_pipe.read_to_end(&mut buf);
        buf
    });
    let err_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = err_pipe.read_to_end(&mut buf);
        buf
    });

    let status = match child.wait_timeout(timeout).map_err(ExecError::Io)? {
        Some(status) => status,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            // Return promptly; the detached drain threads finish once the killed child's pipes
            // close. A shell-wrapper process that spawns un-exec'd grandchildren can keep a pipe
            // open — full process-tree teardown (process groups) is the isolation phase (see
            // BACKLOG); this bounds the invocation, which is the contract here.
            return Err(ExecError::TimedOut);
        }
    };

    let stdout = String::from_utf8_lossy(&out_handle.join().unwrap_or_default()).into_owned();
    let stderr = String::from_utf8_lossy(&err_handle.join().unwrap_or_default()).into_owned();

    Ok(Output {
        exit_code: status.code(),
        stdout,
        stderr,
    })
}
