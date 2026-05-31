//! Integration and error-path tests for [`netsuke::ninja_gen`].
//!
//! Exercises [`netsuke::ninja_gen::generate`] and
//! [`netsuke::ninja_gen::generate_into`] error handling alongside scenarios
//! that drive a real `ninja` binary against generated files.

use anyhow::{Context, Result, bail, ensure};
use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use netsuke::ast::Recipe;
use netsuke::ir::{Action, BuildEdge, BuildGraph};
use netsuke::ninja_gen::{NinjaGenError, generate, generate_into};
use rstest::{fixture, rstest};
use std::process::Command;
use tempfile::TempDir;
use test_support::ninja_gen::{self, AssertionType, NinjaIntegrationCase};

/// Provide a temporary directory when Ninja is available, skipping otherwise.
#[fixture]
fn ninja_integration_setup() -> Option<TempDir> {
    ninja_gen::ninja_integration_setup()
}

/// Integration scenarios to confirm Ninja executes commands correctly.
#[rstest]
#[case::multiline_script_valid(NinjaIntegrationCase {
    action: Action {
        recipe: Recipe::Script { script: "echo one\necho two".into() },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    },
    edge: BuildEdge {
        action_id: "script".into(),
        inputs: Vec::new(),
        implicit_deps: Vec::new(),
        explicit_outputs: vec![Utf8PathBuf::from("out")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    },
    target_name: Utf8PathBuf::from("out"),
    ninja_args: vec!["-n"],
    assertion: AssertionType::StatusSuccess,
})]
#[case::script_with_percent(NinjaIntegrationCase {
    action: Action {
        recipe: Recipe::Script { script: "echo 100% > out".into() },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    },
    edge: BuildEdge {
        action_id: "percent".into(),
        inputs: Vec::new(),
        implicit_deps: Vec::new(),
        explicit_outputs: vec![Utf8PathBuf::from("out")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    },
    target_name: Utf8PathBuf::from("out"),
    ninja_args: vec!["out"],
    assertion: AssertionType::FileContent("100%".into()),
})]
#[case::script_with_backtick(NinjaIntegrationCase {
    action: Action {
        recipe: Recipe::Script { script: "echo `echo hi` > out".into() },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    },
    edge: BuildEdge {
        action_id: "tick".into(),
        inputs: Vec::new(),
        implicit_deps: Vec::new(),
        explicit_outputs: vec![Utf8PathBuf::from("out")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    },
    target_name: Utf8PathBuf::from("out"),
    ninja_args: vec!["out"],
    assertion: AssertionType::FileContent("hi".into()),
})]
#[case::phony_action_executes_command(NinjaIntegrationCase {
    action: Action {
        recipe: Recipe::Command { command: "touch action-called.txt".into() },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    },
    edge: BuildEdge {
        action_id: "hello".into(),
        inputs: Vec::new(),
        implicit_deps: Vec::new(),
        explicit_outputs: vec![Utf8PathBuf::from("say-hello")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: true,
        always: false,
    },
    target_name: Utf8PathBuf::from("action-called.txt"),
    ninja_args: vec!["say-hello"],
    assertion: AssertionType::FileExists,
})]
fn ninja_integration_tests(
    ninja_integration_setup: Option<TempDir>,
    #[case] case: NinjaIntegrationCase,
) -> Result<()> {
    let Some(dir) = ninja_integration_setup else {
        return Ok(());
    };
    let dir_path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf())
        .map_err(|path| anyhow::anyhow!("temp dir path {:?} is not UTF-8", path))?;

    let NinjaIntegrationCase {
        action,
        edge,
        target_name,
        ninja_args,
        assertion,
    } = case;

    let output = edge
        .explicit_outputs
        .first()
        .context("edge should have at least one explicit output")?
        .clone();
    if !matches!(&assertion, AssertionType::FileExists) {
        ensure!(
            output == target_name,
            "expected edge output '{}' to match test target '{target_name}'",
            output
        );
    }
    let mut graph = BuildGraph::default();
    graph.actions.insert(edge.action_id.clone(), action);
    graph.targets.insert(output.clone(), edge);
    graph.default_targets.push(output);

    let ninja = generate(&graph)?;
    let handle = Dir::open_ambient_dir(&dir_path, ambient_authority())
        .with_context(|| format!("open ambient dir for temp workspace at {dir_path}"))?;
    handle
        .write("build.ninja", ninja.as_bytes())
        .context("write ninja build file")?;
    let status = Command::new("ninja")
        .args(&ninja_args)
        .current_dir(dir_path.as_std_path())
        .status()
        .context("invoke ninja")?;

    match assertion {
        AssertionType::StatusSuccess => {
            ensure!(status.success(), "ninja invocation should succeed");
        }
        AssertionType::FileExists => {
            ensure!(status.success(), "ninja invocation should succeed");
            let exists = handle
                .try_exists(target_name.as_str())
                .with_context(|| format!("check existence of {target_name}"))?;
            ensure!(
                exists,
                "expected {} to exist after ninja invocation",
                &target_name
            );
        }
        AssertionType::FileContent(expected) => {
            ensure!(status.success(), "ninja invocation should succeed");
            let content = handle
                .read_to_string(target_name.as_str())
                .with_context(|| format!("read target file {target_name}"))?;
            ensure!(
                content.trim() == expected,
                "expected file content '{}', got '{}'",
                expected,
                content.trim()
            );
        }
    }
    Ok(())
}

#[rstest]
fn errors_when_action_missing() -> Result<()> {
    let mut graph = BuildGraph::default();
    let edge = BuildEdge {
        action_id: "missing".into(),
        inputs: Vec::new(),
        implicit_deps: Vec::new(),
        explicit_outputs: vec![Utf8PathBuf::from("out")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    };
    graph.targets.insert(Utf8PathBuf::from("out"), edge);
    let Err(err) = generate(&graph) else {
        bail!("expected missing action to error");
    };
    ensure!(
        matches!(err, NinjaGenError::MissingAction { ref id, .. } if id == "missing"),
        "unexpected error variant: {err:?}"
    );
    Ok(())
}

#[rstest]
fn generate_format_error() -> Result<()> {
    use std::fmt::{self, Write};

    struct FailWriter;
    impl Write for FailWriter {
        fn write_str(&mut self, _: &str) -> fmt::Result {
            Err(fmt::Error)
        }
    }

    let action = Action {
        recipe: Recipe::Command {
            command: "true".into(),
        },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    };
    let edge = BuildEdge {
        action_id: "a".into(),
        inputs: Vec::new(),
        implicit_deps: Vec::new(),
        explicit_outputs: vec![Utf8PathBuf::from("out")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    };
    let mut graph = BuildGraph::default();
    graph.actions.insert("a".into(), action);
    graph.targets.insert(Utf8PathBuf::from("out"), edge);

    let mut writer = FailWriter;
    let Err(err) = generate_into(&graph, &mut writer) else {
        bail!("expected format error when writer fails");
    };
    ensure!(
        matches!(err, NinjaGenError::Format { .. }),
        "unexpected error: {err:?}"
    );
    Ok(())
}
