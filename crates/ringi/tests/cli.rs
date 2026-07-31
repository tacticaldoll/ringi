//! End-to-end tests that drive the real `ringi` binary: `init` scaffolds a config, and `run`
//! loads it and drives a run over a fixture workspace, mapping convergence to the exit status.
//! This is the app running end to end (DoD), not a library-level test.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A unique scratch directory for one test, holding the fixture scripts and `ringi.toml`.
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

#[test]
fn run_converges_and_exits_success_when_verification_is_green() {
    let dir = scratch("green");
    let agent = fake_agent(&dir);
    let check = write_script(&dir, "check.sh", "exit 0");
    write_config(&dir, &agent, &check);

    let output = ringi()
        .current_dir(&dir)
        .args(["run", "--workspace"])
        .arg(&dir)
        .args(["--task", "ship the widget"])
        .output()
        .expect("ringi run executes");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "a green run must exit success; stdout={stdout}, stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("converged"), "stdout={stdout}");
    assert!(stdout.contains("none open"), "stdout={stdout}");
}

#[test]
fn run_does_not_converge_and_exits_failure_when_verification_is_red() {
    let dir = scratch("red");
    let agent = fake_agent(&dir);
    let check = write_script(&dir, "check.sh", "exit 1");
    write_config(&dir, &agent, &check);

    let output = ringi()
        .current_dir(&dir)
        .args(["run", "--workspace"])
        .arg(&dir)
        .args(["--task", "ship the widget"])
        .output()
        .expect("ringi run executes");

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
    let output = ringi()
        .current_dir(&dir)
        .args(["run", "--workspace"])
        .arg(&dir)
        .args(["--task", "x"])
        .output()
        .expect("ringi run executes");

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
fn init_scaffolds_a_config_and_does_not_clobber() {
    let dir = scratch("init");

    let first = ringi().current_dir(&dir).arg("init").output().unwrap();
    assert!(first.status.success());
    let config_path = dir.join("ringi.toml");
    assert!(config_path.exists(), "init must write ringi.toml");
    let original = std::fs::read_to_string(&config_path).unwrap();

    // A second init must leave the existing file untouched.
    let second = ringi().current_dir(&dir).arg("init").output().unwrap();
    assert!(second.status.success());
    assert_eq!(
        original,
        std::fs::read_to_string(&config_path).unwrap(),
        "init must not clobber an existing config"
    );
    assert!(
        String::from_utf8_lossy(&second.stdout).contains("already exists"),
        "second init should report the file already exists"
    );
}
