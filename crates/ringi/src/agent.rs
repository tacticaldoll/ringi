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
use std::path::PathBuf;
use std::time::Duration;

use crate::exec;

/// Which agent role an invocation is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentRole {
    Respondent,
    Arbitrator,
    ConditionEvaluator,
    // Legacy roles (removed in 8.3)
    Builder,
    Reviewer,
}

/// A request to run an agent.
#[derive(Debug, Clone)]
pub struct AgentRequest {
    /// The role this invocation plays.
    pub role: AgentRole,
    /// A system instruction for the session (e.g. persona or format constraints).
    pub session_instruction: Option<String>,
    /// The prompt handed to the agent (delivered on stdin).
    pub prompt: String,
    /// The working directory the agent runs in.
    pub working_dir: PathBuf,
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
    /// Captured standard output (the natural-language answer).
    pub stdout: String,
    /// Captured standard error.
    pub stderr: String,
    /// Optional adapter-specific transport metadata.
    pub metadata: Option<serde_json::Value>,
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

impl From<exec::ExecError> for AgentError {
    fn from(error: exec::ExecError) -> Self {
        match error {
            exec::ExecError::Spawn(error) => Self::Spawn(error),
            exec::ExecError::TimedOut => Self::TimedOut,
            exec::ExecError::Io(error) => Self::Io(error),
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

fn parse_metadata(stdout: &str) -> Option<serde_json::Value> {
    stdout
        .lines()
        .rev()
        .find_map(|line| serde_json::from_str::<serde_json::Value>(line.trim()).ok())
}

impl AgentAdapter for SubprocessAdapter {
    fn run(&self, request: AgentRequest) -> Result<AgentResponse, AgentError> {
        // Compose the shared subprocess primitive (program+args, never a shell; minimized env;
        // timeout-bounded; concurrent pipe drain). The prompt is delivered on stdin.
        let output = exec::run(
            &self.program,
            &self.args,
            &request.working_dir,
            &request.env,
            &request.prompt,
            request.timeout,
        )?;
        let metadata = parse_metadata(&output.stdout);
        Ok(AgentResponse {
            exit_code: output.exit_code,
            stdout: output.stdout,
            stderr: output.stderr,
            metadata,
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
            role: AgentRole::Respondent,
            session_instruction: None,
            prompt: "do the thing".to_string(),
            working_dir: std::env::temp_dir(),
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
        let metadata = response.metadata.expect("structured output parsed");
        assert_eq!(metadata["status"], "completed");
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
            response.metadata.is_none(),
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
