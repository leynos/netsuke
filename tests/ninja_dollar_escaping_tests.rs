//! Differential tests for shell dollars preserved through the Ninja backend.
//!
//! These tests use Ninja itself as the lexer oracle. The generated command
//! must be the shell text held by the IR after Netsuke lowers `$in` and `$out`.

use anyhow::{Context, Result, bail, ensure};
use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use mockable::{DefaultEnv, Env};
use netsuke::{
    ast::{Recipe, StringOrList},
    ir::{Action, BuildEdge, BuildGraph, DependencyOrder},
    manifest,
    ninja_gen::{RecipeShell, generate_with_shell},
};
use rstest::rstest;
use std::{
    ffi::OsString,
    fmt::{self, Debug, Display, Formatter},
    path::Path,
    process::Command,
};
use tempfile::TempDir;
use test_support::ninja::ninja_integration_workspace;

const SENTINEL: &str = "NETSUKE_TEST_SENTINEL";
const SENTINEL_VALUE: &str = "sentinel-value";

/// Render Ninja content with explicit POSIX shell semantics.
fn generate_posix(graph: &BuildGraph) -> Result<String> {
    generate_with_shell(graph, RecipeShell::Posix).map_err(Into::into)
}
/// Read the host `PATH` for isolated real-Ninja child processes.
///
/// # Errors
///
/// Returns an error if the host environment does not provide `PATH`.
fn host_path() -> Result<OsString> {
    let process_env = DefaultEnv;
    process_env
        .os_string("PATH")
        .context("host PATH is required to run Ninja")
}

/// Build the action used by a generated single-edge test graph.
const fn action(recipe: Recipe) -> Action {
    Action {
        recipe,
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    }
}

/// Build a single-action graph for a command-escaping scenario.
fn graph(recipe: Recipe, input: &str, output: &str) -> BuildGraph {
    let edge = BuildEdge {
        action_id: "action".into(),
        inputs: vec![Utf8PathBuf::from(input)],
        implicit_deps: Vec::new(),
        dependency_order: DependencyOrder::Parallel,
        explicit_outputs: vec![Utf8PathBuf::from(output)],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    };
    let mut graph = BuildGraph::default();
    graph.actions.insert("action".into(), action(recipe));
    graph.targets.insert(Utf8PathBuf::from(output), edge);
    graph.default_targets.push(Utf8PathBuf::from(output));
    graph
}

/// Create an isolated workspace after confirming that Ninja is available.
fn required_ninja_workspace() -> Result<TempDir> {
    ninja_integration_workspace().context("Ninja is required for dollar-escaping tests")
}

/// Render a path with its escaped debug representation for invalid UTF-8 diagnostics.
struct DebugPath<'path>(&'path Path);

impl Display for DebugPath<'_> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(self.0, formatter)
    }
}

/// An isolated workspace containing a generated Ninja file and its output.
struct NinjaWorkspace {
    /// Keeps the temporary directory alive while Ninja uses its contents.
    _workspace: TempDir,
    /// UTF-8 path used as the child Ninja process's current directory.
    path: Utf8PathBuf,
    /// Capability-scoped directory used to read and write test artefacts.
    directory: Dir,
}

impl NinjaWorkspace {
    /// Creates an isolated workspace and writes the generated Ninja file into it.
    fn create(ninja_file: &str) -> anyhow::Result<Self> {
        let temporary_directory = required_ninja_workspace()?;
        let path = Utf8PathBuf::from_path_buf(temporary_directory.path().to_path_buf()).map_err(
            |non_utf8| anyhow::anyhow!("non-UTF-8 temporary path: {}", DebugPath(&non_utf8)),
        )?;
        let directory = Dir::open_ambient_dir(&path, ambient_authority())
            .with_context(|| format!("open Ninja workspace {path}"))?;
        let workspace = Self {
            _workspace: temporary_directory,
            path,
            directory,
        };
        workspace
            .directory
            .write("build.ninja", ninja_file)
            .context("write generated Ninja file")?;
        Ok(workspace)
    }
}
/// Query Ninja's effective commands for a generated target.
fn ninja_commands(ninja_file: &str, target: &str) -> Result<String> {
    let workspace = NinjaWorkspace::create(ninja_file)?;

    let output = Command::new("ninja")
        .args(["-f", "build.ninja", "-t", "commands", target])
        .current_dir(workspace.path.as_std_path())
        .env_clear()
        .env("PATH", host_path()?)
        .env(SENTINEL, SENTINEL_VALUE)
        .output()
        .context("run Ninja command oracle")?;
    if !output.status.success() {
        bail!(
            "Ninja rejected generated file: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8(output.stdout).context("Ninja command output was not UTF-8")
}

/// Run a generated Ninja target and read its shell-produced output.
#[cfg(unix)]
fn ninja_output(
    ninja_file: &str,
    environment_value: Option<&str>,
    input: Option<(&str, &str)>,
) -> Result<String> {
    let workspace = NinjaWorkspace::create(ninja_file)?;
    if let Some((path, contents)) = input {
        workspace
            .directory
            .write(path, contents)
            .context("write script input")?;
    }

    let mut command = Command::new("ninja");
    command
        .args(["-f", "build.ninja", "out"])
        .current_dir(workspace.path.as_std_path())
        .env_clear()
        .env("PATH", host_path()?);
    if let Some(value) = environment_value {
        command.env(SENTINEL, value);
    }
    let output = command.output().context("run generated Ninja build")?;
    if !output.status.success() {
        bail!(
            "Ninja failed to execute generated file: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    workspace
        .directory
        .read_to_string("out")
        .context("read shell output from generated target")
}

/// Verify that remaining shell dollars are doubled for Ninja.
#[rstest]
#[case::shell_variable("echo $NETSUKE_TEST_SENTINEL", "echo $$NETSUKE_TEST_SENTINEL")]
#[case::shell_default(
    "echo ${NETSUKE_TEST_SENTINEL:-fallback}",
    "echo $${NETSUKE_TEST_SENTINEL:-fallback}"
)]
#[case::multiple_shell_variables("echo $RUSTFLAGS-$PATH", "echo $$RUSTFLAGS-$$PATH")]
#[case::unrelated_identifier("echo $input", "echo $$input")]
#[case::literal_dollars("echo $$", "echo $$$$")]
fn backend_doubles_every_residual_shell_dollar(#[case] command: &str, #[case] expected: &str) {
    let ninja = generate_posix(&graph(
        Recipe::Command {
            command: command.into(),
        },
        "in",
        "out",
    ))
    .expect("dollar-containing command should remain valid IR shell text");

    assert!(
        ninja.contains(expected),
        "expected Ninja-safe command {expected:?}, got:\n{ninja}"
    );
}

/// Verify that Netsuke placeholders lower before residual dollars are escaped.
#[rstest]
#[case::scalar(Recipe::Command { command: "echo $NETSUKE_TEST_SENTINEL".into() })]
#[case::command_list(Recipe::Command {
    command: StringOrList::List(vec!["echo ${NETSUKE_TEST_SENTINEL:-fallback}".into()]),
})]
#[case::script(Recipe::Script { script: "echo $NETSUKE_TEST_SENTINEL".into() })]
fn ninja_expands_only_netsuke_placeholders(#[case] recipe: Recipe) -> Result<()> {
    let ninja = generate_posix(&graph(recipe, "in", "out"))?;
    let commands = ninja_commands(&ninja, "out")?;
    ensure!(
        commands.contains(SENTINEL) && commands.contains('$'),
        "Ninja must preserve the shell variable for the child shell:\n{commands}"
    );
    Ok(())
}

/// Verify scalar command recipes preserve child-shell defaults after Ninja execution.
#[cfg(unix)]
#[rstest]
#[case::unset(None, "fallback")]
#[case::set(Some(SENTINEL_VALUE), SENTINEL_VALUE)]
fn shell_default_reaches_the_child_shell(
    #[case] environment_value: Option<&str>,
    #[case] expected: &str,
) -> Result<()> {
    let manifest = manifest::from_str(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: out\n    command: 'printf %s \"${NETSUKE_TEST_SENTINEL:-fallback}\" > $out'\n",
    )?;
    let ninja = generate_posix(&BuildGraph::from_manifest_for_shell(
        &manifest,
        RecipeShell::Posix,
    )?)?;

    let actual = ninja_output(&ninja, environment_value, None)?;
    ensure!(
        actual == expected,
        "expected child shell output {expected:?}, got {actual:?}"
    );
    Ok(())
}

/// Verify command-list recipes preserve braced child-shell defaults through Ninja.
#[cfg(unix)]
#[rstest]
#[case::unset(None, "fallback")]
#[case::set(Some(SENTINEL_VALUE), SENTINEL_VALUE)]
fn command_list_default_reaches_the_child_shell(
    #[case] environment_value: Option<&str>,
    #[case] expected: &str,
) -> Result<()> {
    let manifest = manifest::from_str(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: out\n    command:\n      - 'printf %s \"${NETSUKE_TEST_SENTINEL:-fallback}\" > $out'\n",
    )?;
    let ninja = generate_posix(&BuildGraph::from_manifest_for_shell(
        &manifest,
        RecipeShell::Posix,
    )?)?;

    let actual = ninja_output(&ninja, environment_value, None)?;
    ensure!(
        actual == expected,
        "expected command-list child shell output {expected:?}, got {actual:?}"
    );
    Ok(())
}

/// Verify that manifest placeholder syntax lowers inside scalar and list recipes.
#[rstest]
#[case::scalar("command: 'cat $in > $out'")]
#[case::command_list("command:\n      - 'cat $in > $out'")]
fn placeholder_lowering_precedes_backend_escaping(#[case] recipe: &str) -> Result<()> {
    let manifest = manifest::from_str(&format!(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: output.txt\n    sources: input.txt\n    {recipe}\n"
    ))?;
    let ninja = generate_posix(&BuildGraph::from_manifest_for_shell(
        &manifest,
        RecipeShell::Posix,
    )?)?;
    let commands = ninja_commands(&ninja, "output.txt")?;
    ensure!(
        commands.contains("cat input.txt > output.txt"),
        "Ninja must receive lowered paths rather than placeholders:\n{commands}"
    );
    Ok(())
}

/// Verify complex script syntax survives lowering and real Ninja execution.
#[cfg(unix)]
#[rstest]
fn scripts_lower_placeholders_without_command_parser_validation() -> Result<()> {
    let manifest = manifest::from_str(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: out\n    sources: in\n    script: |\n      cat $in > $out\n      # apostrophe's comment and a heredoc must remain valid\n      cat <<'EOF' >> $out\n      done\n      EOF\n",
    )?;
    let graph = BuildGraph::from_manifest_for_shell(&manifest, RecipeShell::Posix)?;
    let ninja = generate_posix(&graph)?;
    ensure!(
        !ninja.contains("\\$out") && !ninja.contains("$in"),
        "script placeholders must be lowered before backend escaping:\n{ninja}"
    );
    let actual = ninja_output(&ninja, None, Some(("in", "script input")))?;
    ensure!(
        actual == "script inputdone\n",
        "expected script and heredoc output, got {actual:?}"
    );
    Ok(())
}

/// Verify script recipes preserve braced child-shell defaults through Ninja.
#[cfg(unix)]
#[rstest]
#[case::unset(None, "fallback")]
#[case::set(Some(SENTINEL_VALUE), SENTINEL_VALUE)]
fn script_default_reaches_the_child_shell(
    #[case] environment_value: Option<&str>,
    #[case] expected: &str,
) -> Result<()> {
    let manifest = manifest::from_str(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: out\n    script: 'printf %s \"${NETSUKE_TEST_SENTINEL:-fallback}\" > $out'\n",
    )?;
    let ninja = generate_posix(&BuildGraph::from_manifest_for_shell(
        &manifest,
        RecipeShell::Posix,
    )?)?;

    let actual = ninja_output(&ninja, environment_value, None)?;
    ensure!(
        actual == expected,
        "expected script child shell output {expected:?}, got {actual:?}"
    );
    Ok(())
}

/// Verify that script placeholders run against their real input and output paths.
#[cfg(unix)]
#[rstest]
fn script_placeholders_execute_against_real_paths() -> Result<()> {
    let manifest = manifest::from_str(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: out\n    sources: in\n    script: \"printf '%s' $in > $out\"\n",
    )?;
    let ninja = generate_posix(&BuildGraph::from_manifest_for_shell(
        &manifest,
        RecipeShell::Posix,
    )?)?;

    let actual = ninja_output(&ninja, None, Some(("in", "script input")))?;
    ensure!(
        actual == "in",
        "expected script to write the lowered input path \"in\", got {actual:?}"
    );
    Ok(())
}

/// Verify that placeholder-looking text inside backticks is rejected before escaping.
#[rstest]
fn placeholders_inside_backticks_are_rejected_before_backend_escaping() -> Result<()> {
    let manifest = manifest::from_str(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: out\n    sources: in\n    script: 'echo `basename $out`'\n",
    )?;
    let result = BuildGraph::from_manifest(&manifest);
    ensure!(
        result.is_err(),
        "a placeholder protected by backticks must not silently reach the shell"
    );
    Ok(())
}

/// Verify that command control characters cannot inject Ninja syntax.
#[rstest]
#[case::newline("echo safe\nbuild injected: action")]
#[case::carriage_return("echo safe\rbuild injected: action")]
fn command_control_characters_are_rejected(#[case] command: &str) {
    let result = generate_posix(&graph(
        Recipe::Command {
            command: command.into(),
        },
        "in",
        "out",
    ));
    assert!(
        result.is_err(),
        "unsafe control characters must not reach a generated Ninja binding"
    );
}

/// Verify that Ninja-unsafe path characters are rejected during generation.
#[rstest]
#[case::dollar("input$file")]
#[case::space("input file")]
#[case::colon("input:file")]
#[case::pipe("input|file")]
#[case::tab("input\tfile")]
#[case::nul("input\0file")]
#[case::carriage_return("input\rfile")]
#[case::newline("input\nfile")]
fn unsafe_paths_are_rejected(#[case] input: &str) {
    let result = generate_posix(&graph(
        Recipe::Command {
            command: "cat $in > $out".into(),
        },
        input,
        "out",
    ));
    assert!(
        result.is_err(),
        "a Ninja-unsupported path must fail generation rather than corrupt an edge"
    );
}

/// Verify that commands without dollars retain their exact text.
#[rstest]
fn dollar_free_commands_remain_byte_identical() -> Result<()> {
    let ninja = generate_posix(&graph(
        Recipe::Command {
            command: "echo hi".into(),
        },
        "in",
        "out",
    ))?;
    ensure!(
        ninja.contains("  command = echo hi\n"),
        "dollar-free command output must not change:\n{ninja}"
    );
    Ok(())
}
