//! Real-Ninja command-oracle support for Ninja generation properties.

use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use proptest::{prelude::*, test_runner::TestRunner};
use std::{fmt::Write as _, process::Command};
use tempfile::TempDir;
use test_support::ninja::{NinjaWorkspaceError, ninja_integration_workspace};

use crate::{
    ast::StringOrList,
    ir::{BuildEdge, BuildGraph, DependencyOrder},
};

use super::generate_posix;

/// Generate ordinary and explicitly braced scalar shell commands.
pub(super) fn scalar_command_strategy() -> impl Strategy<Value = (String, String)> {
    prop::collection::vec(
        prop_oneof![
            Just("plain"),
            Just("two words"),
            Just("$value"),
            Just("${value:-fallback}"),
            Just("$(command)"),
            Just("'quoted'"),
            Just("\"double\""),
            Just("@!?*-"),
        ],
        1..8,
    )
    .prop_map(|parts| {
        let command = format!("echo {}", parts.join(" "));
        let braced_command = format!("{command} ${{value:-fallback}}");
        (command, braced_command)
    })
}

/// Build the one-action graph used by the real-Ninja scalar command oracle.
pub(super) fn scalar_graph(command: String) -> BuildGraph {
    let mut graph = super::command_graph(StringOrList::String(command));
    graph.targets.insert(
        Utf8PathBuf::from("out"),
        BuildEdge {
            action_id: "action".into(),
            inputs: Vec::new(),
            implicit_deps: Vec::new(),
            dependency_order: DependencyOrder::Parallel,
            explicit_outputs: vec![Utf8PathBuf::from("out")],
            implicit_outputs: Vec::new(),
            order_only_deps: Vec::new(),
            phony: false,
            always: false,
        },
    );
    graph
}

/// Hold one verified workspace for a real-Ninja property oracle.
pub(super) struct NinjaCommandOracle {
    _workspace: TempDir,
    path: Utf8PathBuf,
    directory: Dir,
}

impl NinjaCommandOracle {
    /// Prepare the Ninja oracle or skip when Ninja is optional and unavailable.
    pub(super) fn try_create() -> Result<Option<Self>, TestCaseError> {
        let workspace = match ninja_integration_workspace() {
            Ok(workspace) => workspace,
            Err(
                NinjaWorkspaceError::ProbeSpawn(_)
                | NinjaWorkspaceError::ProbeFailed(_)
                | NinjaWorkspaceError::ProbeTimeout(_),
            ) => return Ok(None),
            Err(error) => {
                return Err(TestCaseError::fail(format!(
                    "create Ninja workspace: {error}"
                )));
            }
        };
        let path =
            Utf8PathBuf::from_path_buf(workspace.path().to_path_buf()).map_err(|non_utf8| {
                TestCaseError::fail(format!("non-UTF-8 temporary path: {non_utf8:?}"))
            })?;
        let directory = Dir::open_ambient_dir(&path, ambient_authority()).map_err(|error| {
            TestCaseError::fail(format!("open Ninja workspace {path}: {error}"))
        })?;
        Ok(Some(Self {
            _workspace: workspace,
            path,
            directory,
        }))
    }

    /// Ask Ninja to parse generated files and return their expanded command text.
    pub(super) fn ninja_commands(
        &self,
        ninja_files: &[String],
    ) -> Result<Vec<String>, TestCaseError> {
        let ninja_file = batch_ninja_files(ninja_files)?;
        self.directory
            .write("build.ninja", &ninja_file)
            .map_err(|error| TestCaseError::fail(format!("write generated Ninja file: {error}")))?;

        let targets = (0..ninja_files.len())
            .map(|index| format!("out-{index}"))
            .collect::<Vec<_>>();
        let output = Command::new("ninja")
            .arg("-f")
            .arg("build.ninja")
            .arg("-t")
            .arg("commands")
            .args(&targets)
            .current_dir(self.path.as_std_path())
            .output()
            .map_err(|error| TestCaseError::fail(format!("run Ninja command oracle: {error}")))?;
        if !output.status.success() {
            return Err(TestCaseError::fail(format!(
                "Ninja rejected generated file: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        String::from_utf8(output.stdout)
            .map(|stdout| stdout.lines().map(str::to_owned).collect())
            .map_err(|error| {
                TestCaseError::fail(format!("Ninja command output was not UTF-8: {error}"))
            })
    }
}

/// Combine generated scalar Ninja files into one command-oracle invocation.
fn batch_ninja_files(ninja_files: &[String]) -> Result<String, TestCaseError> {
    let mut batch = String::new();
    for (index, ninja_file) in ninja_files.iter().enumerate() {
        let command = ninja_file
            .lines()
            .find_map(|line| line.strip_prefix("  command = "))
            .ok_or_else(|| TestCaseError::fail("generated scalar Ninja file had no command"))?;
        writeln!(batch, "rule action-{index}")
            .map_err(|error| TestCaseError::fail(format!("write rule: {error}")))?;
        writeln!(batch, "  command = {command}")
            .map_err(|error| TestCaseError::fail(format!("write command: {error}")))?;
        writeln!(batch, "build out-{index}: action-{index}")
            .map_err(|error| TestCaseError::fail(format!("write target: {error}")))?;
    }
    Ok(batch)
}

/// Verify that Ninja preserves scalar commands through the explicit POSIX renderer.
#[test]
fn scalar_command_output_matches_ninja_oracle() {
    let prepared_oracle =
        NinjaCommandOracle::try_create().expect("prepare real-Ninja command oracle");
    let Some(oracle) = prepared_oracle else {
        return;
    };
    let mut runner = TestRunner::new(ProptestConfig {
        cases: 128,
        ..ProptestConfig::default()
    });
    runner
        .run(&scalar_command_strategy(), |(command, braced_command)| {
            prop_assert!(
                braced_command.contains("${"),
                "braced property input must contain a shell braced expansion"
            );
            let candidates = [&command, &braced_command];
            let ninja_files = candidates
                .iter()
                .map(|candidate| {
                    generate_posix(&scalar_graph((*candidate).clone()))
                        .expect("scalar command should generate")
                })
                .collect::<Vec<_>>();
            let observed = oracle.ninja_commands(&ninja_files)?;
            prop_assert_eq!(observed.len(), candidates.len());
            for (candidate, observed_command) in candidates.iter().zip(observed) {
                prop_assert_eq!(observed_command, candidate.as_str());
            }
            Ok(())
        })
        .expect("real-Ninja command oracle property should hold");
}
