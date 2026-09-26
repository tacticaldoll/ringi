//! Executable architectural governance for the ringi workspace.
//!
//! Ringi is an application: it performs I/O by design and carries no sans-I/O teeth. The gate
//! holds three things:
//!
//! - the seam discipline of `docs/domain-language.md`, under which each composed brick's
//!   vocabulary is confined to its seam module;
//! - the agent seam's use of the `crate::exec` subprocess primitive, with no call under
//!   `std::process::Command` of its own in the library root; and
//! - the gate's own independence from the graph it judges.
//!
//! Tianheng judges an external-crate confinement in every compiled root, so the seam boundaries
//! observe both the library root (`src/lib.rs`) and the binary root (`src/main.rs`). In each root
//! a brick may be imported only from a module at its seam's path; the binary root declares none,
//! so a `use` import of a brick anywhere it reaches is outside its seam.

#![forbid(unsafe_code)]

use std::{env, process::ExitCode};

use tianheng::prelude::*;

const SUUNTA_SEAM_REASON: &str = "suunta's vocabulary (Bearing, Sigil, Sounding, ...) enters ringi's library and binary roots only through the convergence seam: in each of those roots, no module outside `crate::convergence` makes a `use` import of suunta. Coverage is partial: a module the binary root itself declares at the seam's path is permitted like the library's seam, and a fully qualified inline path, an `extern crate` declaration, a `use` inside a macro body, and a re-export through the seam are invisible to this import scan, so whether a ringi domain type names suunta's vocabulary stays review-governed — see docs/domain-language.md's seam rule";
const PACTA_SEAM_REASON: &str = "pacta's vocabulary (Pact, Claim, Retainer, Registry, lifecycle, ...) enters ringi's library and binary roots only through the registry seam: in each of those roots, no module outside `crate::registry` makes a `use` import of pacta. Coverage is partial: a module the binary root itself declares at the seam's path is permitted like the library's seam, and a fully qualified inline path, an `extern crate` declaration, a `use` inside a macro body, and a re-export through the seam are invisible to this import scan, so whether a ringi domain type names pacta's vocabulary stays review-governed — see docs/domain-language.md's seam rule";
const CADW_SEAM_REASON: &str = "cadw's vocabulary (TargetId, Ledger, Move, Validator, Rejection, ...) enters ringi's library and binary roots only through the residual-ledger seam: in each of those roots, no module outside `crate::residual_ledger` makes a `use` import of cadw. Coverage is partial: a module the binary root itself declares at the seam's path is permitted like the library's seam, and a fully qualified inline path, an `extern crate` declaration, a `use` inside a macro body, and a re-export through the seam are invisible to this import scan, so whether a ringi domain type names cadw's vocabulary stays review-governed — see docs/domain-language.md's seam rule";
const AGENT_SPAWN_REASON: &str = "the agent seam composes the shared subprocess primitive in `crate::exec` rather than hand-rolling a spawn path: in the library root, no module in `crate::agent`'s subtree, its test modules included, calls a function under `std::process::Command` (such as `Command::new`), whether written fully qualified or through a `use` import or alias. A `type` alias or `pub use` of that path inside the subtree also reacts, fail-closed, wherever a glob import can reach it, such as a tests module's `use super::*`. Coverage is partial: otherwise a mention as a type or value does not react; a method called on a `Command` value, a spawn through any other API, and a call a macro constructs from fragments are not observed; modules outside `crate::agent`, and the binary root, which declares no agent module, are not judged by this boundary; so what `crate::exec` itself guarantees (program and arguments, never a shell; a minimized environment; a timeout) stays test- and review-governed";
const GOVERNANCE_REASON: &str = "the governance gate must stay independent of the workspace graph it judges: its normal dependencies are Tianheng's composed adopter surface alone, never an individual governance instrument or a workspace crate under judgment.";

fn constitution() -> Constitution {
    Constitution::new("ringi")
        .boundary(
            ModuleBoundary::in_crate("ringi")
                .module("crate::convergence")
                .confine_external_crate("suunta")
                .because(SUUNTA_SEAM_REASON),
        )
        .boundary(
            ModuleBoundary::in_crate("ringi")
                .module("crate::registry")
                .confine_external_crate("pacta")
                .because(PACTA_SEAM_REASON),
        )
        .boundary(
            ModuleBoundary::in_crate("ringi")
                .module("crate::residual_ledger")
                .confine_external_crate("cadw")
                .because(CADW_SEAM_REASON),
        )
        .boundary(
            ModuleBoundary::in_crate("ringi")
                .module("crate::agent")
                .must_not_call_inline("std::process::Command")
                .because(AGENT_SPAWN_REASON),
        )
        .boundary(
            CrateBoundary::crate_("ringi-governance")
                .restrict_dependencies_to(["tianheng"])
                .because(GOVERNANCE_REASON),
        )
}

fn main() -> ExitCode {
    tianheng::run(&constitution(), env::args().collect::<Vec<_>>())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    use super::*;

    const LAW_PROJECTION_PREAMBLE: &str = "\
# Ringi Tianheng Law Projection

This file is generated from `constitution()` in `crates/ringi-governance/src/main.rs`.
The Rust declaration is authoritative; do not edit the projection by hand.
Regenerate it with `BLESS=1 cargo test -p ringi-governance law_projection_is_fresh`.

";

    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn current_workspace_satisfies_constitution() {
        GovernanceTest::for_constitution(constitution())
            .with_manifest_dir(workspace_root())
            .assert_clean();
    }

    #[test]
    fn every_workspace_crate_is_covered() {
        GovernanceTest::for_constitution(constitution())
            .with_manifest_dir(workspace_root())
            .assert_all_workspace_members_covered();
    }

    #[test]
    fn law_projection_is_fresh() {
        GovernanceTest::for_constitution(constitution())
            .with_manifest_dir(workspace_root())
            .assert_projection_fresh_with_preamble("AGENTS.ringi-law.md", LAW_PROJECTION_PREAMBLE);
    }

    #[test]
    fn suunta_outside_the_convergence_seam_is_rejected() {
        assert_seam_violation("suunta", "convergence", "Bearing");
    }

    #[test]
    fn pacta_outside_the_registry_seam_is_rejected() {
        assert_seam_violation("pacta", "registry", "Pact");
    }

    #[test]
    fn cadw_outside_the_residual_ledger_seam_is_rejected() {
        assert_seam_violation("cadw", "residual_ledger", "Ledger");
    }

    #[test]
    fn brick_imports_inside_their_seams_stay_clean() {
        let workspace = TempWorkspace::new("ringi-governance-seams-clean");
        workspace.write_ringi(&[
            ("convergence", "use suunta::Bearing;\n"),
            ("registry", "use pacta::Pact;\n"),
            ("residual_ledger", "use cadw::Ledger;\n"),
            ("dossier", "pub struct Dossier;\n"),
        ]);

        let outcome = workspace.outcome();
        assert!(
            matches!(outcome, Outcome::Clean(_)),
            "brick imports inside their seams must raise no violation: {outcome:?}"
        );
    }

    /// The binary root declares no seam module, so a brick import in it, or in a module it
    /// reaches, is outside every seam; the library's seams stay clean beside it.
    #[test]
    fn brick_imports_in_the_binary_root_are_rejected() {
        let workspace = TempWorkspace::new("ringi-governance-binary-root-leak");
        workspace.write_ringi(&[("convergence", "use suunta::Bearing;\n")]);
        workspace.write_source(
            "ringi",
            "main.rs",
            "mod cli;\n\nuse cadw::Ledger;\nuse suunta::Bearing;\n\nfn main() {}\n",
        );
        workspace.write_source("ringi", "cli.rs", "use pacta::Pact;\n");

        let report = workspace.violations();
        for (brick, finding, file) in [
            ("suunta", "crate", "main.rs"),
            ("cadw", "crate", "main.rs"),
            ("pacta", "crate::cli", "cli.rs"),
        ] {
            assert!(
                report.violations.iter().any(|violation| {
                    violation.target() == brick
                        && violation.finding == finding
                        && violation
                            .file
                            .as_deref()
                            .is_some_and(|path| path.ends_with(file))
                }),
                "expected the {brick} seam boundary to fire for {finding} in the binary root: {report:?}"
            );
        }
        assert!(
            !report
                .violations
                .iter()
                .any(|violation| violation.finding == "crate::convergence"),
            "the library's convergence seam must stay clean: {report:?}"
        );
    }

    /// The seam reasons leave two routes into the binary root unobserved: a fully qualified
    /// inline path and a re-export through the library's seam. Both stay clean, so a Tianheng
    /// release that starts observing either one fails here and the reasons are revisited.
    #[test]
    fn inline_paths_and_seam_reexports_in_the_binary_root_stay_unobserved() {
        let workspace = TempWorkspace::new("ringi-governance-binary-root-unobserved");
        workspace.write_ringi(&[("convergence", "pub use suunta::Bearing;\n")]);
        workspace.write_source(
            "ringi",
            "main.rs",
            "use ringi::convergence::Bearing;\n\n\
             fn inline() -> Option<suunta::Bearing> {\n    None\n}\n\n\
             fn main() {\n    let _: Option<Bearing> = inline();\n}\n",
        );

        let outcome = workspace.outcome();
        assert!(
            matches!(outcome, Outcome::Clean(_)),
            "an inline path and a seam re-export in the binary root must raise no violation: \
             {outcome:?}"
        );
    }

    /// A `Command` built in the agent seam fires whether it is reached through an aliased `use`
    /// import or written fully qualified.
    #[test]
    fn a_spawn_hand_rolled_in_the_agent_seam_is_rejected() {
        for (case, agent) in [
            (
                "alias",
                "use std::process::Command as Spawn;\n\npub fn run() {\n    let _ = Spawn::new(\"agent\").status();\n}\n",
            ),
            (
                "qualified",
                "pub fn run() {\n    let _ = std::process::Command::new(\"agent\");\n}\n",
            ),
        ] {
            let workspace = TempWorkspace::new(&format!("ringi-governance-agent-spawn-{case}"));
            workspace.write_ringi(&[("agent", agent)]);

            let report = workspace.violations();
            assert!(
                report.violations.iter().any(|violation| {
                    violation.target() == "std::process::Command"
                        && violation.finding == "std::process::Command::new in crate::agent"
                        && violation
                            .file
                            .as_deref()
                            .is_some_and(|path| path.ends_with("agent.rs"))
                }),
                "expected the agent spawn boundary to fire for the {case} spawn: {report:?}"
            );
        }
    }

    /// Shaped like the real `agent.rs`: a tests module globbing its parent with `use super::*`
    /// makes a `type` alias of `Command` in the seam react fail-closed, even though the alias is
    /// only ever used as a type.
    #[test]
    fn a_command_alias_reachable_through_a_glob_is_rejected() {
        let workspace = TempWorkspace::new("ringi-governance-agent-glob-alias");
        workspace.write_ringi(&[(
            "agent",
            "type Spawner = std::process::Command;\n\npub fn run(spawner: &mut Spawner) {\n    let _ = spawner.spawn();\n}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n}\n",
        )]);

        let report = workspace.violations();
        assert!(
            report.violations.iter().any(|violation| {
                violation.target() == "std::process::Command"
                    && violation.finding == "glob super in crate::agent"
                    && violation
                        .file
                        .as_deref()
                        .is_some_and(|path| path.ends_with("agent.rs"))
            }),
            "expected the agent spawn boundary to fire on the glob-reachable alias: {report:?}"
        );
    }

    /// The agent seam composing `crate::exec`, which alone builds the `Command`, stays clean, as do
    /// other `std::process` items in the seam; the binary root is not judged by this boundary.
    #[test]
    fn composing_the_exec_primitive_stays_clean() {
        let workspace = TempWorkspace::new("ringi-governance-agent-exec-clean");
        workspace.write_ringi(&[
            (
                "exec",
                "use std::process::Command;\n\npub fn run(program: &str) {\n    let _ = Command::new(program);\n}\n",
            ),
            (
                "agent",
                "use crate::exec;\n\npub fn run() -> std::process::ExitCode {\n    let _ = std::process::id();\n    exec::run(\"agent\");\n    std::process::ExitCode::SUCCESS\n}\n",
            ),
        ]);
        workspace.write_source(
            "ringi",
            "main.rs",
            "fn main() {\n    let _ = std::process::Command::new(\"agent\");\n}\n",
        );

        let outcome = workspace.outcome();
        assert!(
            matches!(outcome, Outcome::Clean(_)),
            "composing crate::exec from the agent seam must raise no violation: {outcome:?}"
        );
    }

    #[test]
    fn governance_dependency_beyond_tianheng_is_rejected() {
        let workspace = TempWorkspace::new("ringi-governance-extra-dependency");
        workspace.write_ringi(&[]);
        workspace.write_package(
            "ringi-governance",
            "[dependencies]\ntianheng = { path = \"../tianheng\" }\nsuunta = { path = \"../suunta\" }\n",
            &[("lib.rs", "")],
        );

        let report = workspace.violations();
        assert!(
            report.violations.iter().any(|violation| {
                violation.target() == "ringi-governance" && violation.finding == "suunta"
            }),
            "expected the governance dependency boundary to fire: {report:?}"
        );
    }

    /// Place a brick import in `dossier`, outside the brick's `seam`, while the seam itself imports
    /// the brick legitimately; exactly the out-of-seam import must fire.
    fn assert_seam_violation(brick: &str, seam: &str, item: &str) {
        let workspace = TempWorkspace::new(&format!("ringi-governance-{brick}-leak"));
        let seam_source = format!("use {brick}::{item};\n");
        let leak_source = format!("use {brick}::{item};\npub struct Dossier;\n");
        workspace.write_ringi(&[
            (seam, seam_source.as_str()),
            ("dossier", leak_source.as_str()),
        ]);

        let report = workspace.violations();
        assert!(
            report.violations.iter().any(|violation| {
                violation.target() == brick && violation.finding == "crate::dossier"
            }),
            "expected the {brick} seam boundary to fire for crate::dossier: {report:?}"
        );
    }

    /// A scratch workspace holding stand-ins for the three bricks, the ringi application crate with
    /// the modules a test writes, and the governance crate, so every boundary has a real target.
    struct TempWorkspace {
        path: PathBuf,
    }

    impl TempWorkspace {
        fn new(name: &str) -> Self {
            let path = env::temp_dir().join(format!("{name}-{}", std::process::id()));
            if path.exists() {
                fs::remove_dir_all(&path).expect("stale temporary workspace should be removable");
            }
            let workspace = Self { path };
            for brick in ["tianheng", "suunta", "pacta", "cadw"] {
                workspace.write_package(
                    brick,
                    "",
                    &[(
                        "lib.rs",
                        "pub struct Bearing;\npub struct Pact;\npub struct Ledger;\n",
                    )],
                );
            }
            workspace.write_package(
                "ringi-governance",
                "[dependencies]\ntianheng = { path = \"../tianheng\" }\n",
                &[("lib.rs", "")],
            );
            fs::write(
                workspace.path.join("Cargo.toml"),
                "[workspace]\nresolver = \"2\"\nmembers = [\"tianheng\", \"suunta\", \"pacta\", \"cadw\", \"ringi\", \"ringi-governance\"]\n",
            )
            .expect("workspace manifest should be writable");
            workspace
        }

        /// Write the ringi crate with every seam module and the agent module present (empty unless
        /// given) plus the given extra modules.
        fn write_ringi(&self, modules: &[(&str, &str)]) {
            let mut names = vec!["agent", "convergence", "registry", "residual_ledger"];
            for (module, _) in modules {
                if !names.contains(module) {
                    names.push(module);
                }
            }
            let lib = names
                .iter()
                .map(|module| format!("pub mod {module};\n"))
                .collect::<String>();
            let mut sources = vec![("lib.rs".to_owned(), lib)];
            for module in &names {
                let body = modules
                    .iter()
                    .find(|(name, _)| name == module)
                    .map(|(_, body)| (*body).to_owned())
                    .unwrap_or_default();
                sources.push((format!("{module}.rs"), body));
            }
            let sources = sources
                .iter()
                .map(|(file, body)| (file.as_str(), body.as_str()))
                .collect::<Vec<_>>();
            self.write_package(
                "ringi",
                "[dependencies]\nsuunta = { path = \"../suunta\" }\npacta = { path = \"../pacta\" }\ncadw = { path = \"../cadw\" }\n",
                &sources,
            );
        }

        /// Add one source file to a package already written.
        fn write_source(&self, package: &str, file: &str, source: &str) {
            fs::write(self.path.join(package).join("src").join(file), source)
                .expect("package source should be writable");
        }

        fn outcome(&self) -> Outcome {
            tianheng::check_constitution(&constitution(), &self.path.join("Cargo.toml"))
        }

        fn violations(&self) -> Report {
            match self.outcome() {
                Outcome::Violations(report) => report,
                other => panic!("expected violations, got {other:?}"),
            }
        }

        fn write_package(&self, name: &str, dependencies: &str, sources: &[(&str, &str)]) {
            let package = self.path.join(name);
            let _ = fs::remove_dir_all(&package);
            fs::create_dir_all(package.join("src")).expect("package source dir should be writable");
            fs::write(
                package.join("Cargo.toml"),
                format!(
                    "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n{dependencies}"
                ),
            )
            .expect("package manifest should be writable");
            for (file, source) in sources {
                fs::write(package.join("src").join(file), source)
                    .expect("package source should be writable");
            }
        }
    }

    impl Drop for TempWorkspace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
