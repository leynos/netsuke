//! Property coverage for rendered interpreter paths.
//!
//! Windows resolves an unqualified executable name through its search order, so
//! a bound path must survive three independent decoders: Ninja's own variable
//! and escape expansion, the `CommandLineToArgvW` argument splitter, and the
//! recipe transport. These properties generate paths with spaces, Unicode,
//! literal dollars and the platform separator, render them through each
//! transport, and check the decoded argument vector with the independent oracle
//! in `test_support::windows_argv`, the independent parser in
//! `test_support::ninja_semantics`, and a real `ninja` binary.

use super::super::{
    InterpreterExecutable, RecipeShell, generate_with_interpreter,
    test_support::{command_graph, command_graph_with},
};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use proptest::prelude::*;
use proptest::test_runner::{TestCaseError, TestRunner};
use std::process::Command;
use test_support::ninja::ninja_integration_workspace;
use test_support::ninja_semantics::{GeneratedNinja, RecipeNeedle, RecipeTransport};
use test_support::windows_argv;

/// Recipe text long enough to select the PowerShell response-file transport.
///
/// The Base64 encoding of a UTF-16LE script this size exceeds Windows'
/// command-line limit on its own, so the decision cannot depend on the
/// interpreter path supplied by the property.
const OVERSIZED_RECIPE_CHARACTERS: usize = 30_000;

/// One generated plan and the target Ninja should expand.
struct Plan<'a> {
    /// Generated Ninja text.
    ninja: &'a str,
    /// Build target whose command Ninja should print.
    target: &'a str,
}

impl Plan<'_> {
    /// The first `command` binding in the generated plan.
    fn command(&self) -> &str {
        self.ninja
            .lines()
            .find_map(|line| line.strip_prefix("  command = "))
            .expect("generated manifest should carry one command binding")
    }
}

/// The separator Windows arguments and the host both accept in a fixture.
const fn separator() -> char {
    if cfg!(windows) { '\\' } else { '/' }
}

/// Generate path segments that exercise quoting, Unicode and Ninja escapes.
fn path_segments() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(
        prop_oneof![
            Just("Program Files".to_owned()),
            Just("two words".to_owned()),
            Just("$dollar".to_owned()),
            Just("\u{03bb}ambda".to_owned()),
            "[A-Za-z0-9_.-]{1,8}".prop_map(|segment| segment),
        ],
        1..4,
    )
}

/// Build an absolute interpreter path from `segments`, padded to `target_len`.
fn absolute_path(segments: &[String], target_len: usize) -> String {
    let separator = separator();
    let root = if cfg!(windows) { "C:\\" } else { "/" };
    let fixed = root.len() + segments.iter().map(|s| s.len() + 1).sum::<usize>() + 15;
    let padding = "a".repeat(target_len.saturating_sub(fixed));
    let mut path = String::from(root);
    for segment in segments {
        path.push_str(segment);
        path.push(separator);
    }
    path.push_str(&padding);
    path.push(separator);
    path.push_str("powershell.exe");
    path
}

/// Build one executable from an absolute fixture path.
fn executable(path: &str) -> InterpreterExecutable {
    InterpreterExecutable::from_absolute_path(Utf8PathBuf::from(path))
        .expect("generated fixture paths should be absolute")
}

/// Assert the generated PowerShell command decodes to `path` and its transport.
fn assert_power_shell_arguments(
    plan: &Plan<'_>,
    path: &str,
    response_file: bool,
) -> Result<(), TestCaseError> {
    let arguments = windows_argv::split(&plan.command().replace("$$", "$"));
    prop_assert_eq!(
        arguments.first().map(String::as_str),
        Some(path),
        "the bound interpreter should decode as the first Windows argument"
    );
    if response_file {
        prop_assert_eq!(
            arguments
                .get(arguments.len().saturating_sub(2))
                .map(String::as_str),
            Some("-File"),
            "the large-recipe transport should pass -File"
        );
        prop_assert_eq!(
            arguments.last().map(String::as_str),
            Some("$rspfile"),
            "the response-file path should stay a Ninja variable reference"
        );
    } else {
        prop_assert!(
            arguments
                .iter()
                .any(|argument| argument == "-EncodedCommand"),
            "the inline transport should pass -EncodedCommand"
        );
    }
    Ok(())
}

/// Assert the generated Bash command decodes to `path` and the recipe text.
fn assert_bash_arguments(plan: &Plan<'_>, path: &str) -> Result<(), TestCaseError> {
    let arguments = windows_argv::split(&plan.command().replace("$$", "$"));
    prop_assert_eq!(
        arguments.first().map(String::as_str),
        Some(path),
        "the bound interpreter should decode as the first Windows argument"
    );
    prop_assert_eq!(
        arguments.get(1).map(String::as_str),
        Some("-e"),
        "the Bash transport should pass -e"
    );
    prop_assert_eq!(
        arguments.get(2).map(String::as_str),
        Some("-c"),
        "the Bash transport should pass -c"
    );
    prop_assert_eq!(
        arguments.get(3).map(String::as_str),
        Some("echo hi"),
        "the recipe text should decode as one Windows argument"
    );
    Ok(())
}

/// Render `plan` into `directory` and return the command Ninja expands.
fn ninja_commands(
    directory: &Dir,
    workspace: &Utf8Path,
    plan: &Plan<'_>,
) -> Result<String, TestCaseError> {
    directory
        .write("build.ninja", plan.ninja)
        .map_err(|error| TestCaseError::fail(format!("write plan: {error}")))?;
    let output = Command::new("ninja")
        .arg("-f")
        .arg("build.ninja")
        .arg("-t")
        .arg("commands")
        .arg(plan.target)
        .current_dir(workspace.as_std_path())
        .output()
        .map_err(|error| TestCaseError::fail(format!("run Ninja: {error}")))?;
    if !output.status.success() {
        return Err(TestCaseError::fail(format!(
            "Ninja rejected the plan: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let commands = String::from_utf8(output.stdout)
        .map_err(|error| TestCaseError::fail(format!("Ninja output: {error}")))?;
    Ok(commands
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or_default()
        .to_owned())
}

/// The bound path survives Ninja parsing and Windows quoting for every
/// PowerShell transport.
#[test]
fn bound_power_shell_paths_survive_ninja_parsing() {
    let mut runner = TestRunner::new(ProptestConfig {
        cases: 48,
        ..ProptestConfig::default()
    });
    runner
        .run(
            &(
                path_segments(),
                prop_oneof![0usize..64, 32_590usize..32_610],
            ),
            |(segments, target_len)| {
                let path = absolute_path(&segments, target_len);
                let ninja = generate_with_interpreter(
                    &command_graph(),
                    RecipeShell::PowerShell,
                    &executable(&path),
                )
                .map_err(|error| TestCaseError::fail(format!("generate: {error}")))?;
                let plan = Plan {
                    ninja: &ninja,
                    target: "out",
                };
                let parsed = GeneratedNinja::new(&ninja);
                prop_assert!(
                    parsed
                        .recipe_contains(RecipeNeedle::new("echo hi"))
                        .map_err(|error| TestCaseError::fail(format!("decode: {error}")))?,
                    "the independent parser should recover the recipe text"
                );
                let response_file = plan.command().contains("-File \"$rspfile\"");
                let expected = if response_file {
                    RecipeTransport::PowerShellResponseFile
                } else {
                    RecipeTransport::PowerShellEncodedCommand
                };
                prop_assert_eq!(parsed.detected_recipe_transports(), vec![expected]);
                prop_assert_eq!(
                    response_file,
                    target_len > 32_000,
                    "a path beyond the command-line limit should select the response file"
                );
                assert_power_shell_arguments(&plan, &path, response_file)?;
                Ok(())
            },
        )
        .expect("PowerShell interpreter path property should hold");
}

/// The bound path survives Ninja parsing and Windows quoting for Bash.
#[test]
fn bound_bash_paths_survive_ninja_parsing() {
    let mut runner = TestRunner::new(ProptestConfig {
        cases: 48,
        ..ProptestConfig::default()
    });
    runner
        .run(&(path_segments(), 0usize..64), |(segments, target_len)| {
            let path = absolute_path(&segments, target_len);
            let ninja =
                generate_with_interpreter(&command_graph(), RecipeShell::Bash, &executable(&path))
                    .map_err(|error| TestCaseError::fail(format!("generate: {error}")))?;
            let plan = Plan {
                ninja: &ninja,
                target: "out",
            };
            let parsed = GeneratedNinja::new(&ninja);
            prop_assert!(
                parsed
                    .recipe_contains(RecipeNeedle::new("echo hi"))
                    .map_err(|error| TestCaseError::fail(format!("decode: {error}")))?,
                "the independent parser should recover the Bash recipe text"
            );
            prop_assert_eq!(
                parsed.detected_recipe_transports(),
                vec![RecipeTransport::Plaintext]
            );
            assert_bash_arguments(&plan, &path)?;
            Ok(())
        })
        .expect("Bash interpreter path property should hold");
}

/// A real Ninja binary expands the generated plan back to the same path.
#[test]
fn bound_paths_survive_real_ninja() {
    let Ok(workspace) = ninja_integration_workspace() else {
        return;
    };
    let Some(workspace_path) = Utf8Path::from_path(workspace.path()) else {
        panic!("the Ninja integration workspace path should be UTF-8");
    };
    let directory = Dir::open_ambient_dir(workspace_path, ambient_authority())
        .expect("the Ninja integration workspace should open as a capability directory");
    let mut runner = TestRunner::new(ProptestConfig {
        cases: 32,
        ..ProptestConfig::default()
    });
    runner
        .run(
            &(path_segments(), 0usize..64, any::<bool>()),
            |(segments, target_len, power_shell)| {
                let path = absolute_path(&segments, target_len);
                let shell = if power_shell {
                    RecipeShell::PowerShell
                } else {
                    RecipeShell::Bash
                };
                let ninja = generate_with_interpreter(&command_graph(), shell, &executable(&path))
                    .map_err(|error| TestCaseError::fail(format!("generate: {error}")))?;
                let plan = Plan {
                    ninja: &ninja,
                    target: "out",
                };
                let expanded = ninja_commands(&directory, workspace_path, &plan)?;
                prop_assert!(
                    expanded.contains(&path),
                    "Ninja should expand the escaped plan back to the bound path"
                );
                let arguments = windows_argv::split(&expanded);
                prop_assert_eq!(
                    arguments.first().map(String::as_str),
                    Some(path.as_str()),
                    "the expanded command should decode to the bound path first"
                );
                Ok(())
            },
        )
        .expect("real-Ninja interpreter path property should hold");
}

/// An oversized recipe with an ordinary interpreter path keeps the
/// response-file transport intact through real Ninja expansion.
#[test]
fn bound_oversized_recipes_survive_real_ninja() {
    let Ok(workspace) = ninja_integration_workspace() else {
        return;
    };
    let Some(workspace_path) = Utf8Path::from_path(workspace.path()) else {
        panic!("the Ninja integration workspace path should be UTF-8");
    };
    let directory = Dir::open_ambient_dir(workspace_path, ambient_authority())
        .expect("the Ninja integration workspace should open as a capability directory");
    let recipe = format!("echo {}", "a".repeat(OVERSIZED_RECIPE_CHARACTERS));
    let mut runner = TestRunner::new(ProptestConfig {
        cases: 8,
        ..ProptestConfig::default()
    });
    runner
        .run(&path_segments(), |segments| {
            let path = absolute_path(&segments, 0);
            let ninja = generate_with_interpreter(
                &command_graph_with(&recipe),
                RecipeShell::PowerShell,
                &executable(&path),
            )
            .map_err(|error| TestCaseError::fail(format!("generate: {error}")))?;
            let plan = Plan {
                ninja: &ninja,
                target: "out",
            };
            prop_assert!(
                plan.command().contains("-File \"$rspfile\""),
                "an oversized recipe should select the response-file transport"
            );
            prop_assert_eq!(
                GeneratedNinja::new(&ninja).detected_recipe_transports(),
                vec![RecipeTransport::PowerShellResponseFile]
            );
            let expanded = ninja_commands(&directory, workspace_path, &plan)?;
            let arguments = windows_argv::split(&expanded);
            prop_assert_eq!(
                arguments.first().map(String::as_str),
                Some(path.as_str()),
                "the expanded command should decode to the bound path first"
            );
            let flag = arguments
                .iter()
                .position(|argument| argument == "-File")
                .ok_or_else(|| {
                    TestCaseError::fail("Ninja should expand -File for the response file")
                })?;
            prop_assert!(
                arguments
                    .get(flag + 1)
                    .is_some_and(|argument| argument.starts_with("out.netsuke-")),
                "the expanded command should name the generated response file"
            );
            Ok(())
        })
        .expect("real-Ninja oversized recipe property should hold");
}
