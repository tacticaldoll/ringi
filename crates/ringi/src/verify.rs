//! The production [`Verification`]: certify the goal by running config-supplied commands.
//!
//! This is ringi's own blood — the objective certifier behind the core invariant *tool
//! verification outranks model opinion*. Ringi re-runs the checks itself and computes the
//! verdict from their exit statuses; no agent output can set or override it. Each command is
//! spawned through the shared [`crate::exec`] primitive (program + args, never a shell,
//! timeout-bounded), so verification and agent invocation compose one hardened spawn path.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use crate::exec;
use crate::reconcile::{Verdict, Verification};

/// A single verification command: a program and its argument vector, bounded by a timeout.
///
/// Supplied as program + args (never a shell command line), so the "never through a shell"
/// invariant holds with no tokenization and no argument injection.
#[derive(Debug, Clone)]
pub struct VerifyCommand {
    /// The program to run.
    pub program: String,
    /// The arguments, passed literally (no shell interpretation).
    pub args: Vec<String>,
    /// The wall-clock bound on this command.
    pub timeout: Duration,
}

impl VerifyCommand {
    /// A command running `program` with `args`, bounded by `timeout`.
    #[must_use]
    pub fn new(program: impl Into<String>, args: Vec<String>, timeout: Duration) -> Self {
        Self {
            program: program.into(),
            args,
            timeout,
        }
    }
}

/// Certifies the goal by running its configured commands in the workspace. The verdict is
/// [`Verdict::Pass`] iff every command exits zero; any non-zero exit, spawn failure, or timeout
/// is [`Verdict::Fail`]. The commands are config-supplied; ringi does not decide their content.
#[derive(Debug, Clone)]
pub struct CommandVerification {
    commands: Vec<VerifyCommand>,
    workspace: PathBuf,
}

impl CommandVerification {
    /// A verifier that runs `commands` in `workspace`.
    #[must_use]
    pub fn new(commands: Vec<VerifyCommand>, workspace: impl Into<PathBuf>) -> Self {
        Self {
            commands,
            workspace: workspace.into(),
        }
    }
}

impl Verification for CommandVerification {
    /// Run every configured command; the state on disk (which a builder round mutates) is the
    /// input, so the same commands are re-run each round. `round` is unused: the verdict is a
    /// function of the current workspace, not the round number.
    fn verify(&self, _round: usize) -> Verdict {
        for command in &self.commands {
            // A clean exit passes this command; a non-zero exit, spawn failure, or timeout
            // fails the whole verdict. Short-circuit: no later command can flip Fail to Pass,
            // and a failed check never raises to the loop — it drives another round.
            match exec::run(
                &command.program,
                &command.args,
                &self.workspace,
                &HashMap::new(),
                "",
                command.timeout,
            ) {
                Ok(output) if output.exit_code == Some(0) => {}
                _ => return Verdict::Fail,
            }
        }
        Verdict::Pass
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::reconcile::{Finding, ReviewRunner, RoundBuilder, StepOutcome, run_rounds};
    use pacta_memory::MemoryRegistry;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    fn script_in(dir: &Path, name: &str, body: &str) -> PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn workspace(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("ringi-verify-{}-{tag}", std::process::id()))
    }

    fn cmd(path: &Path) -> VerifyCommand {
        VerifyCommand::new(
            path.to_string_lossy().to_string(),
            Vec::new(),
            Duration::from_secs(5),
        )
    }

    #[test]
    fn all_commands_pass_is_green() {
        let ws = workspace("green");
        let a = script_in(&ws, "a.sh", "exit 0");
        let b = script_in(&ws, "b.sh", "exit 0");
        let verifier = CommandVerification::new(vec![cmd(&a), cmd(&b)], ws.clone());
        assert_eq!(verifier.verify(0), Verdict::Pass, "all-zero exits => Pass");
    }

    #[test]
    fn any_failing_command_is_red() {
        let ws = workspace("red");
        let ok = script_in(&ws, "ok.sh", "exit 0");
        let bad = script_in(&ws, "bad.sh", "exit 1");
        let verifier = CommandVerification::new(vec![cmd(&ok), cmd(&bad)], ws.clone());
        assert_eq!(
            verifier.verify(0),
            Verdict::Fail,
            "a non-zero exit fails the whole verdict"
        );
    }

    #[test]
    fn a_spawn_failure_is_red() {
        let ws = workspace("nospawn");
        std::fs::create_dir_all(&ws).unwrap();
        // A program that does not exist cannot be spawned -> Fail, not an error to the caller.
        let missing = VerifyCommand::new(
            ws.join("does-not-exist").to_string_lossy().to_string(),
            Vec::new(),
            Duration::from_secs(5),
        );
        let verifier = CommandVerification::new(vec![missing], ws.clone());
        assert_eq!(verifier.verify(0), Verdict::Fail, "spawn failure => Fail");
    }

    #[test]
    fn a_timeout_is_red() {
        let ws = workspace("timeout");
        let hang = script_in(&ws, "hang.sh", "exec sleep 30");
        let slow = VerifyCommand::new(
            hang.to_string_lossy().to_string(),
            Vec::new(),
            Duration::from_millis(200),
        );
        let verifier = CommandVerification::new(vec![slow], ws.clone());
        assert_eq!(verifier.verify(0), Verdict::Fail, "a timeout => Fail");
    }

    #[test]
    fn commands_are_spawned_without_a_shell() {
        let ws = workspace("noshell");
        // Exits 0 only if it received exactly one argument equal to the literal metacharacter
        // string. Spawned via a shell, the `;` would split it and the check would fail; spawned
        // as program+args, the argument arrives literally -> Pass proves no shell involvement.
        let check = script_in(
            &ws,
            "check.sh",
            "[ \"$#\" -eq 1 ] && [ \"$1\" = 'x; echo INJECTED' ] && exit 0\nexit 1",
        );
        let literal = VerifyCommand::new(
            check.to_string_lossy().to_string(),
            vec!["x; echo INJECTED".to_string()],
            Duration::from_secs(5),
        );
        let verifier = CommandVerification::new(vec![literal], ws.clone());
        assert_eq!(
            verifier.verify(0),
            Verdict::Pass,
            "the metacharacter argument must arrive literally (no shell)"
        );
    }

    // A scripted Builder for the integration test: every round "runs".
    struct ScriptBuild;
    impl RoundBuilder for ScriptBuild {
        fn build(&self, _round: usize) -> StepOutcome {
            StepOutcome::Succeeded
        }
    }

    // A Reviewer that never surfaces findings.
    struct CleanReview;
    impl ReviewRunner for CleanReview {
        fn review(&self, _round: usize) -> Vec<Finding> {
            Vec::new()
        }
    }

    #[test]
    fn command_verification_drives_the_round_loop_to_convergence() {
        // CommandVerification composes with run_rounds like any Verification: a green command
        // set makes the goal Satisfied, and with no open findings the run converges.
        let ws = workspace("rounds");
        let green = script_in(&ws, "green.sh", "exit 0");
        let verifier = CommandVerification::new(vec![cmd(&green)], ws.clone());
        let report = run_rounds(
            "run-verify",
            &ScriptBuild,
            &CleanReview,
            &verifier,
            MemoryRegistry::seeded,
        );
        assert!(
            report.converged,
            "a green command verdict must drive convergence: {report:?}"
        );
    }
}
