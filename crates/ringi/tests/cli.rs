//! End-to-end tests that drive the real `ringi` binary: `init` provisions the store and scaffolds
//! a config; `run` loads it, drives a run over the durable store, records the outcome, and maps
//! convergence to the exit status; `status` reads a persisted run — including from a fresh process,
//! which is the durability proof. This is the app running end to end (DoD).
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A unique scratch directory for one test, holding the fixture scripts, `ringi.toml`, and the
/// `.ringi/` store (the binary runs with this as its working directory).
fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ringi-cli-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_script(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

fn write_config(dir: &Path, agent: &Path, check: &Path) {
    let config = format!(
        "[agent]\nprogram = {agent:?}\nargs = []\ntimeout_secs = 5\n\n\
         [[verification]]\nprogram = {check:?}\nargs = []\ntimeout_secs = 5\n",
    );
    std::fs::write(dir.join("ringi.toml"), config).unwrap();
}

/// A fake Agent CLI: as Reviewer (its prompt says "Review") it surfaces no findings; otherwise it
/// acts as the Builder and reports a build. A run over it converges iff verification is green.
fn fake_agent(dir: &Path) -> PathBuf {
    write_script(
        dir,
        "agent.sh",
        "p=$(cat); case \"$p\" in \
         *Review*) echo '{\"findings\":[]}';; \
         *) echo '{\"status\":\"built\"}';; esac",
    )
}

fn ringi() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ringi"))
}

/// The run id printed on the first line of a run's output, as "run <id>".
fn run_id_from(stdout: &str) -> String {
    stdout
        .lines()
        .next()
        .and_then(|l| l.strip_prefix("run "))
        .expect("first line is 'run <id>'")
        .trim()
        .to_string()
}

/// Set up a scratch dir with a fixture agent and a check of the given exit code.
fn fixture(tag: &str, check_exit: u8) -> PathBuf {
    let dir = scratch(tag);
    let agent = fake_agent(&dir);
    let check = write_script(&dir, "check.sh", &format!("exit {check_exit}"));
    write_config(&dir, &agent, &check);
    dir
}

fn run_in(dir: &Path) -> std::process::Output {
    ringi()
        .current_dir(dir)
        .args(["run", "--workspace"])
        .arg(dir)
        .args(["--task", "ship the widget"])
        .output()
        .expect("ringi run executes")
}

#[test]
fn run_converges_and_exits_success_when_verification_is_green() {
    let dir = fixture("green", 0);
    let output = run_in(&dir);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "a green run must exit success; stdout={stdout}, stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("converged in"), "stdout={stdout}");
    assert!(stdout.contains("none open"), "stdout={stdout}");
}

#[test]
fn run_does_not_converge_and_exits_failure_when_verification_is_red() {
    let dir = fixture("red", 1);
    let output = run_in(&dir);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !output.status.success(),
        "a red run must exit non-zero; stdout={stdout}"
    );
    assert!(stdout.contains("did not converge"), "stdout={stdout}");
}

#[test]
fn run_fails_closed_when_config_is_missing() {
    let dir = scratch("noconfig");
    let output = run_in(&dir);
    assert!(
        !output.status.success(),
        "a missing config must fail, not run with defaults"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("ringi.toml"),
        "stderr should name the file: {stderr}"
    );
}

#[test]
fn a_recorded_run_is_readable_by_status_in_a_fresh_process() {
    let dir = fixture("durable", 0);

    // Process 1: drive the run (records it into .ringi/state.sqlite).
    let run = run_in(&dir);
    assert!(run.status.success());
    let run_id = run_id_from(&String::from_utf8_lossy(&run.stdout));

    // Process 2: a fresh invocation reads the persisted run — the store is the source of truth.
    let status = ringi()
        .current_dir(&dir)
        .args(["status", &run_id])
        .output()
        .expect("ringi status executes");
    let stdout = String::from_utf8_lossy(&status.stdout);
    assert!(
        status.status.success(),
        "status of a recorded run succeeds; stdout={stdout}, stderr={}",
        String::from_utf8_lossy(&status.stderr)
    );
    assert!(
        stdout.contains(&run_id),
        "status shows the run id: {stdout}"
    );
    assert!(stdout.contains("converged in"), "stdout={stdout}");
}

#[test]
fn a_failed_run_is_recorded_and_status_reports_it() {
    let dir = fixture("durable-red", 1);
    let run = run_in(&dir);
    assert!(!run.status.success());
    let run_id = run_id_from(&String::from_utf8_lossy(&run.stdout));

    let status = ringi()
        .current_dir(&dir)
        .args(["status", &run_id])
        .output()
        .expect("ringi status executes");
    let stdout = String::from_utf8_lossy(&status.stdout);
    assert!(status.status.success(), "status itself succeeds: {stdout}");
    assert!(stdout.contains("did not converge"), "stdout={stdout}");
}

#[test]
fn status_of_an_unknown_run_fails_clearly() {
    let dir = fixture("unknown", 0);
    let status = ringi()
        .current_dir(&dir)
        .args(["status", "no-such-run"])
        .output()
        .expect("ringi status executes");
    assert!(
        !status.status.success(),
        "status of an unknown id must exit non-zero"
    );
    let stderr = String::from_utf8_lossy(&status.stderr);
    assert!(stderr.contains("no run with id"), "stderr={stderr}");
}

#[test]
fn init_provisions_the_store_and_scaffolds_config_without_clobber() {
    let dir = scratch("init");

    let first = ringi().current_dir(&dir).arg("init").output().unwrap();
    assert!(first.status.success());
    assert!(
        dir.join(".ringi").join("state.sqlite").exists(),
        "init must provision the store"
    );
    let config_path = dir.join("ringi.toml");
    assert!(config_path.exists(), "init must write ringi.toml");
    let original = std::fs::read_to_string(&config_path).unwrap();

    // A second init must leave the existing config untouched.
    let second = ringi().current_dir(&dir).arg("init").output().unwrap();
    assert!(second.status.success());
    assert_eq!(
        original,
        std::fs::read_to_string(&config_path).unwrap(),
        "init must not clobber an existing config"
    );
    assert!(
        String::from_utf8_lossy(&second.stdout).contains("already exists"),
        "second init should report the config already exists"
    );
}
