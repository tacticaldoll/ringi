//! Load a run's configuration from a TOML file and turn it into a [`RunConfig`].
//!
//! The workspace and the task come from the command line; everything else — the Agent CLI, the
//! verification commands, and the agent timeout — comes from `ringi.toml`, so one config serves
//! many runs that differ only in workspace and task. Loading **fails closed**: a missing,
//! unreadable, or malformed file is an error, never a silently defaulted or partial run.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Context;
use serde::Deserialize;
use uuid::Uuid;

use crate::run::RunConfig;
use crate::verify::VerifyCommand;

/// The config file name, resolved in the current directory.
pub const CONFIG_FILE: &str = "ringi.toml";

/// A commented default config that `ringi init` writes for a user to fill in. Valid TOML for the
/// expected shape; the user replaces the placeholder Agent CLI before running.
pub const DEFAULT_CONFIG: &str = r#"# ringi run configuration.
#
# The workspace and the task are given on the command line:
#   ringi run --workspace <path> --task "<what to do>"
# Everything below is shared across runs.

# The Agent CLI ringi drives for both the Builder and Reviewer roles. It is spawned as
# program + args, never through a shell. Replace the placeholder with your Agent CLI.
[agent]
program = "your-agent-cli"
args = []
timeout_secs = 300

# Objective checks ringi re-runs itself to certify the goal is met. The goal is done only when
# every command exits zero — an agent's claim of success never counts. Add one [[verification]]
# table per command.
[[verification]]
program = "cargo"
args = ["test"]
timeout_secs = 600
"#;

/// The on-disk run configuration: the parameters not supplied on the command line.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunFileConfig {
    agent: AgentConfig,
    #[serde(default)]
    verification: Vec<CommandConfig>,
}

/// The Agent CLI to drive, and the wall-clock bound on each invocation.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentConfig {
    program: String,
    #[serde(default)]
    args: Vec<String>,
    timeout_secs: u64,
}

/// One objective verification command: a program, its arguments, and a timeout.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandConfig {
    program: String,
    #[serde(default)]
    args: Vec<String>,
    timeout_secs: u64,
}

/// Read and parse the config file at `path`. Fails closed: a missing/unreadable file or TOML that
/// does not match the expected shape is an error naming the problem, never a defaulted run.
pub fn load(path: &Path) -> anyhow::Result<RunFileConfig> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading config file {}", path.display()))?;
    let config = toml::from_str(&text)
        .with_context(|| format!("parsing config file {} as TOML", path.display()))?;
    Ok(config)
}

impl RunFileConfig {
    /// Combine the file config with the command-line `workspace` and `task` into a [`RunConfig`].
    /// The `run_id` is a deterministic UUIDv5 over the workspace and task (identity is cosmetic
    /// for an ephemeral in-memory run; durable identity is a later phase).
    #[must_use]
    pub fn into_run_config(self, workspace: PathBuf, task: String) -> RunConfig {
        let run_id = Uuid::new_v5(
            &Uuid::NAMESPACE_OID,
            format!("{}\n{task}", workspace.display()).as_bytes(),
        )
        .to_string();
        RunConfig {
            run_id,
            workspace,
            agent_program: self.agent.program,
            agent_args: self.agent.args,
            task,
            verification: self
                .verification
                .into_iter()
                .map(|c| VerifyCommand::new(c.program, c.args, Duration::from_secs(c.timeout_secs)))
                .collect(),
            timeout: Duration::from_secs(self.agent.timeout_secs),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_well_formed_config_parses_and_converts() {
        let toml = r#"
            [agent]
            program = "agent"
            args = ["--json"]
            timeout_secs = 120

            [[verification]]
            program = "cargo"
            args = ["test"]
            timeout_secs = 300

            [[verification]]
            program = "cargo"
            args = ["clippy"]
            timeout_secs = 60
        "#;
        let file: RunFileConfig = toml::from_str(toml).expect("valid config parses");
        let config = file.into_run_config(PathBuf::from("/tmp/ws"), "do it".to_string());

        assert_eq!(config.agent_program, "agent");
        assert_eq!(config.agent_args, vec!["--json"]);
        assert_eq!(config.task, "do it");
        assert_eq!(config.timeout, Duration::from_secs(120));
        assert_eq!(config.verification.len(), 2);
        assert_eq!(config.verification[0].program, "cargo");
        assert_eq!(config.verification[0].args, vec!["test"]);
        assert_eq!(config.verification[1].timeout, Duration::from_secs(60));
        assert!(!config.run_id.is_empty());
    }

    #[test]
    fn the_run_id_is_deterministic_over_workspace_and_task() {
        let mk = || {
            toml::from_str::<RunFileConfig>("[agent]\nprogram = \"a\"\ntimeout_secs = 1\n")
                .unwrap()
                .into_run_config(PathBuf::from("/ws"), "task".to_string())
                .run_id
        };
        assert_eq!(mk(), mk(), "same workspace + task yields the same run id");
    }

    #[test]
    fn malformed_toml_is_an_error() {
        assert!(
            toml::from_str::<RunFileConfig>("this is not = valid = toml").is_err(),
            "malformed TOML must not parse"
        );
    }

    #[test]
    fn missing_required_field_is_an_error() {
        // No `timeout_secs` under [agent] -> the shape does not match, so parsing fails closed.
        assert!(
            toml::from_str::<RunFileConfig>("[agent]\nprogram = \"a\"\n").is_err(),
            "a missing required field must be an error, not a default"
        );
    }

    #[test]
    fn unknown_field_is_rejected() {
        let toml = "[agent]\nprogram = \"a\"\ntimeout_secs = 1\nbogus = true\n";
        assert!(
            toml::from_str::<RunFileConfig>(toml).is_err(),
            "an unknown field is a likely typo and must be rejected, not ignored"
        );
    }

    #[test]
    fn a_missing_file_fails_closed() {
        let result = load(Path::new("/no/such/ringi.toml"));
        assert!(result.is_err(), "a missing config file must be an error");
    }

    #[test]
    fn the_default_config_is_valid_for_the_expected_shape() {
        assert!(
            toml::from_str::<RunFileConfig>(DEFAULT_CONFIG).is_ok(),
            "init's scaffold must itself parse as a valid config"
        );
    }
}
