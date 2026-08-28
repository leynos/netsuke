//! Real-Ninja command-oracle support for Ninja generation properties.

use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use proptest::prelude::*;
use std::process::Command;
use tempfile::TempDir;
use test_support::ninja::{NinjaWorkspaceError, ninja_integration_workspace};

use crate::{
    ast::StringOrList,
    ir::{BuildEdge, BuildGraph, DependencyOrder},
};

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

    /// Ask Ninja to parse a generated file and return its expanded command text.
    pub(super) fn ninja_commands(&self, ninja_file: &str) -> Result<String, TestCaseError> {
        self.directory
            .write("build.ninja", ninja_file)
            .map_err(|error| TestCaseError::fail(format!("write generated Ninja file: {error}")))?;

        let output = Command::new("ninja")
            .args(["-f", "build.ninja", "-t", "commands", "out"])
            .current_dir(self.path.as_std_path())
            .output()
            .map_err(|error| TestCaseError::fail(format!("run Ninja command oracle: {error}")))?;
        if !output.status.success() {
            return Err(TestCaseError::fail(format!(
                "Ninja rejected generated file: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        String::from_utf8(output.stdout).map_err(|error| {
            TestCaseError::fail(format!("Ninja command output was not UTF-8: {error}"))
        })
    }
}
