//! The agent seam: invoke an Agent CLI as a subprocess and read its structured output.
//!
//! This is ringi's own blood — the uniform way it drives heterogeneous Agent CLIs, so the
//! orchestration never knows a specific CLI's flags. The transport does not judge outcomes:
//! it reports the exit code and best-effort structured output; whether that is acceptable is
//! the caller's decision.
//!
//! Synchronous by design (v1 runs one run at a time); async is deferred until concurrency
//! forces it. The one security invariant not deferred: the agent is spawned as a program
//! with arguments, never through a shell.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use wait_timeout::ChildExt;

/// Which agent role an invocation is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentRole {
    /// Proposes changes.
    Builder,
    /// Scrutinizes changes; read-only.
    Reviewer,
}

/// A request to run an agent.
#[derive(Debug, Clone)]
pub struct AgentRequest {
    /// The role this invocation plays.
    pub role: AgentRole,
    /// The prompt handed to the agent (delivered on stdin).
    pub prompt: String,
    /// The working directory the agent runs in.
    pub workspace: PathBuf,
    /// The wall-clock bound on the invocation.
    pub timeout: Duration,
    /// Extra environment variables to expose (added to a minimized base).
    pub env: HashMap<String, String>,
}

/// The result of running an agent. The agent *ran*; whether its outcome is acceptable is the
/// caller's judgment.
#[derive(Debug, Clone)]
pub struct AgentResponse {
    /// The process exit code, if the process exited normally.
    pub exit_code: Option<i32>,
    /// Captured standard output.
    pub stdout: String,
    /// Captured standard error.
    pub stderr: String,
    /// The agent's structured output, parsed best-effort from stdout.
    pub structured: Option<serde_json::Value>,
}

/// An infrastructure failure running an agent — distinct from the agent producing a bad
/// outcome (which is reported in [`AgentResponse`], not raised).
#[derive(Debug)]
pub enum AgentError {
    /// The process could not be spawned.
    Spawn(std::io::Error),
    /// The agent did not exit within its timeout and was terminated.
    TimedOut,
    /// An I/O failure while communicating with the process.
    Io(std::io::Error),
}

impl std::fmt::Display for AgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spawn(error) => write!(f, "failed to spawn agent: {error}"),
            Self::TimedOut => write!(f, "agent did not finish within its timeout"),
            Self::Io(error) => write!(f, "agent I/O error: {error}"),
        }
    }
}

impl std::error::Error for AgentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Spawn(error) | Self::Io(error) => Some(error),
            Self::TimedOut => None,
        }
    }
}

/// The seam by which ringi invokes an Agent CLI. Callers depend on this, never on a specific
/// CLI's flags.
pub trait AgentAdapter {
    /// Run the agent for `request`, returning its response or an infrastructure error.
    fn run(&self, request: AgentRequest) -> Result<AgentResponse, AgentError>;
}

/// Runs an Agent CLI as a subprocess: a fixed `program` and `args`, spawned directly (never
/// through a shell), in the request's workspace, bounded by the request's timeout.
#[derive(Debug, Clone)]
pub struct SubprocessAdapter {
    program: String,
    args: Vec<String>,
}

impl SubprocessAdapter {
    /// An adapter that runs `program` with `args`.
    #[must_use]
    pub fn new(program: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            program: program.into(),
            args,
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

/// Parse the agent's structured output: the last line of stdout that is a valid JSON value
/// (agents may print logs before it). Best-effort — absence yields `None`.
fn parse_structured(stdout: &str) -> Option<serde_json::Value> {
    stdout
        .lines()
        .rev()
        .find_map(|line| serde_json::from_str::<serde_json::Value>(line.trim()).ok())
}

impl AgentAdapter for SubprocessAdapter {
    fn run(&self, request: AgentRequest) -> Result<AgentResponse, AgentError> {
        // program + args, never a shell.
        let mut child = Command::new(&self.program)
            .args(&self.args)
            .current_dir(&request.workspace)
            .env_clear()
            .envs(minimal_base_env())
            .envs(&request.env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(AgentError::Spawn)?;

        // Service all three pipes on their own threads so none can fill and deadlock the
        // child: feeding the prompt to stdin, and draining stdout/stderr, all proceed
        // concurrently while the main thread waits on the timeout.
        let mut stdin_pipe = child.stdin.take().expect("stdin piped");
        let prompt = request.prompt;
        std::thread::spawn(move || {
            let _ = stdin_pipe.write_all(prompt.as_bytes());
            // stdin_pipe dropped here -> closed, so an agent reading stdin sees EOF.
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

        let status = match child
            .wait_timeout(request.timeout)
            .map_err(AgentError::Io)?
        {
            Some(status) => status,
            None => {
                let _ = child.kill();
                let _ = child.wait();
                // Return promptly; the detached drain threads finish once the killed child's
                // pipes close. A shell-wrapper agent that spawns un-exec'd grandchildren can
                // keep a pipe open — full process-tree teardown (process groups) is the
                // isolation phase (see BACKLOG); this bounds the invocation, which is the
                // contract here.
                return Err(AgentError::TimedOut);
            }
        };

        let stdout = String::from_utf8_lossy(&out_handle.join().unwrap_or_default()).into_owned();
        let stderr = String::from_utf8_lossy(&err_handle.join().unwrap_or_default()).into_owned();
        let structured = parse_structured(&stdout);

        Ok(AgentResponse {
            exit_code: status.code(),
            stdout,
            stderr,
            structured,
        })
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    // Write an executable fake-agent script and return its path.
    fn fake_agent(name: &str, body: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ringi-agent-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn request() -> AgentRequest {
        AgentRequest {
            role: AgentRole::Builder,
            prompt: "do the thing".to_string(),
            workspace: std::env::temp_dir(),
            timeout: Duration::from_secs(5),
            env: HashMap::new(),
        }
    }

    fn adapter(script: &Path) -> SubprocessAdapter {
        SubprocessAdapter::new(script.to_string_lossy().to_string(), Vec::new())
    }

    #[test]
    fn success_with_structured_output() {
        let script = fake_agent(
            "ok.sh",
            "echo 'log: starting'\necho '{\"status\":\"completed\",\"summary\":\"ok\"}'",
        );
        let response = adapter(&script).run(request()).expect("runs");
        assert_eq!(response.exit_code, Some(0));
        let structured = response.structured.expect("structured output parsed");
        assert_eq!(structured["status"], "completed");
    }

    #[test]
    fn non_zero_exit_is_reported_not_raised() {
        let script = fake_agent("fail.sh", "echo 'boom' 1>&2\nexit 3");
        let response = adapter(&script)
            .run(request())
            .expect("runs (non-zero is not an error)");
        assert_eq!(response.exit_code, Some(3));
        assert!(response.stderr.contains("boom"));
    }

    #[test]
    fn malformed_output_yields_no_structured_value() {
        let script = fake_agent("garble.sh", "echo 'not json at all'");
        let response = adapter(&script).run(request()).expect("runs");
        assert_eq!(response.exit_code, Some(0));
        assert!(
            response.structured.is_none(),
            "no valid JSON -> no structured value"
        );
        assert!(response.stdout.contains("not json"));
    }

    #[test]
    fn a_hung_agent_times_out() {
        let script = fake_agent("hang.sh", "exec sleep 30");
        let mut req = request();
        req.timeout = Duration::from_millis(200);
        let result = adapter(&script).run(req);
        assert!(
            matches!(result, Err(AgentError::TimedOut)),
            "hung agent must time out"
        );
    }
}
