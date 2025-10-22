#![allow(
    clippy::expect_used,
    reason = "integration tests prefer expect for expressive failures"
)]

//! Unit tests for Ninja file generation.
//!
//! Snapshot tests use `insta` to ensure the emitted manifest remains stable.
//! Tests cover various scenarios including phony targets, standard builds with
//! multiple inputs and outputs, complex dependency relationships, and edge
//! cases like empty build graphs.

use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use insta::{Settings, assert_snapshot};
use netsuke::ast::Recipe;
use netsuke::ir::{Action, BuildEdge, BuildGraph};
use netsuke::ninja_gen::{NinjaGenError, generate, generate_into};
use rstest::{fixture, rstest};
use std::process::Command;
use tempfile::{TempDir, tempdir};

fn skip_if_ninja_unavailable() -> bool {
    match Command::new("ninja").arg("--version").output() {
        Err(_) => {
            tracing::warn!("skipping test: ninja not found in PATH");
            true
        }
        Ok(output) if !output.status.success() => {
            tracing::warn!("skipping test: ninja --version failed");
            true
        }
        Ok(_) => false,
    }
}

/// Define how the integration test should assert Ninja's behaviour.
#[derive(Debug)]
enum AssertionType {
    FileContent(String),
    FileExists,
    StatusSuccess,
}

/// Provide a temporary directory when Ninja is available, skipping otherwise.
#[fixture]
fn ninja_integration_setup() -> Option<TempDir> {
    if skip_if_ninja_unavailable() {
        None
    } else {
        Some(tempdir().expect("temp dir"))
    }
}

#[rstest]
#[case::phony_target_runs_command(
    Action {
        recipe: Recipe::Command { command: "true".into() },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    },
    BuildEdge {
        action_id: "a".into(),
        inputs: vec![Utf8PathBuf::from("in")],
        explicit_outputs: vec![Utf8PathBuf::from("out")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: true,
        always: false,
    },
    Utf8PathBuf::from("out"),
    concat!(
        "rule a\n",
        "  command = true\n\n",
        "build out: a in\n\n",
    ),
)]
#[case::standard_build(
    Action {
        recipe: Recipe::Command { command: "cc -c 'a.c' 'b.c' -o 'ab.o'".into() },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    },
    BuildEdge {
        action_id: "compile".into(),
        inputs: vec![Utf8PathBuf::from("a.c"), Utf8PathBuf::from("b.c")],
        explicit_outputs: vec![Utf8PathBuf::from("ab.o")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    },
    Utf8PathBuf::from("ab.o"),
    concat!(
        "rule compile\n",
        "  command = cc -c 'a.c' 'b.c' -o 'ab.o'\n\n",
        "build ab.o: compile a.c b.c\n\n",
    ),
)]
#[case::complex_dependencies(
    Action {
        recipe: Recipe::Command { command: "true".into() },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    },
    BuildEdge {
        action_id: "b".into(),
        inputs: vec![Utf8PathBuf::from("in")],
        explicit_outputs: vec![Utf8PathBuf::from("out"), Utf8PathBuf::from("log")],
        implicit_outputs: vec![Utf8PathBuf::from("out.d")],
        order_only_deps: vec![Utf8PathBuf::from("stamp")],
        phony: false,
        always: false,
    },
    Utf8PathBuf::from("out"),
    concat!(
        "rule b\n",
        "  command = true\n\n",
        "build out log | out.d: b in || stamp\n\n",
    ),
)]
fn generate_ninja_scenarios(
    #[case] action: Action,
    #[case] edge: BuildEdge,
    #[case] target_path: Utf8PathBuf,
    #[case] expected: &str,
) {
    let mut graph = BuildGraph::default();
    graph.actions.insert(edge.action_id.clone(), action);
    graph.targets.insert(target_path, edge);

    let ninja = generate(&graph).expect("generate ninja");
    assert_eq!(ninja, expected);
}

#[rstest]
fn generate_empty_graph() {
    let graph = BuildGraph::default();
    let ninja = generate(&graph).expect("generate ninja");
    assert!(ninja.is_empty());
}

#[rstest]
fn generate_multiline_script_snapshot() {
    let mut graph = BuildGraph::default();
    graph.actions.insert(
        "script".into(),
        Action {
            recipe: Recipe::Script {
                script: "echo one\necho two".into(),
            },
            description: None,
            depfile: None,
            deps_format: None,
            pool: None,
            restat: false,
        },
    );
    graph.targets.insert(
        Utf8PathBuf::from("out"),
        BuildEdge {
            action_id: "script".into(),
            inputs: Vec::new(),
            explicit_outputs: vec![Utf8PathBuf::from("out")],
            implicit_outputs: Vec::new(),
            order_only_deps: Vec::new(),
            phony: false,
            always: false,
        },
    );
    graph.default_targets.push(Utf8PathBuf::from("out"));

    let ninja = generate(&graph).expect("generate ninja");
    let mut settings = Settings::new();
    settings.set_snapshot_path(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/snapshots/ninja",
    ));
    settings.bind(|| {
        assert_snapshot!("multiline_script_ninja", &ninja);
    });
    assert!(
        ninja.contains("printf %b") && ninja.contains("\\n"),
        "script should use printf %b with encoded newlines",
    );
}

/// Integration scenarios to confirm Ninja executes commands correctly.
#[rstest]
#[case::multiline_script_valid(
    Action {
        recipe: Recipe::Script { script: "echo one\necho two".into() },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    },
    BuildEdge {
        action_id: "script".into(),
        inputs: Vec::new(),
        explicit_outputs: vec![Utf8PathBuf::from("out")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    },
    Utf8PathBuf::from("out"),
    vec!["-n"],
    AssertionType::StatusSuccess,
)]
#[case::script_with_percent(
    Action {
        recipe: Recipe::Script { script: "echo 100% > out".into() },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    },
    BuildEdge {
        action_id: "percent".into(),
        inputs: Vec::new(),
        explicit_outputs: vec![Utf8PathBuf::from("out")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    },
    Utf8PathBuf::from("out"),
    vec!["out"],
    AssertionType::FileContent("100%".into()),
)]
#[case::script_with_backtick(
    Action {
        recipe: Recipe::Script { script: "echo `echo hi` > out".into() },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    },
    BuildEdge {
        action_id: "tick".into(),
        inputs: Vec::new(),
        explicit_outputs: vec![Utf8PathBuf::from("out")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    },
    Utf8PathBuf::from("out"),
    vec!["out"],
    AssertionType::FileContent("hi".into()),
)]
#[case::phony_action_executes_command(
    Action {
        recipe: Recipe::Command { command: "touch action-called.txt".into() },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    },
    BuildEdge {
        action_id: "hello".into(),
        inputs: Vec::new(),
        explicit_outputs: vec![Utf8PathBuf::from("say-hello")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: true,
        always: false,
    },
    Utf8PathBuf::from("action-called.txt"),
    vec!["say-hello"],
    AssertionType::FileExists,
)]
fn ninja_integration_tests(
    ninja_integration_setup: Option<TempDir>,
    #[case] action: Action,
    #[case] edge: BuildEdge,
    #[case] target_name: Utf8PathBuf,
    #[case] ninja_args: Vec<&str>,
    #[case] assertion: AssertionType,
) {
    let Some(dir) = ninja_integration_setup else {
        return;
    };
    let dir_path =
        Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).expect("utf8 ninja tempdir");

    let output = edge
        .explicit_outputs
        .first()
        .expect("explicit output")
        .clone();
    let mut graph = BuildGraph::default();
    graph.actions.insert(edge.action_id.clone(), action);
    graph.targets.insert(output.clone(), edge);
    graph.default_targets.push(output);

    let ninja = generate(&graph).expect("generate ninja");
    let handle = Dir::open_ambient_dir(&dir_path, ambient_authority()).expect("open ninja tempdir");
    handle
        .write("build.ninja", ninja.as_bytes())
        .expect("write ninja");
    let status = Command::new("ninja")
        .args(&ninja_args)
        .current_dir(dir_path.as_std_path())
        .status()
        .expect("run ninja");

    match assertion {
        AssertionType::StatusSuccess => assert!(status.success()),
        AssertionType::FileExists => {
            assert!(status.success());
            let exists = handle
                .try_exists(target_name.as_str())
                .expect("check target existence");
            assert!(
                exists,
                "expected {} to exist after ninja invocation",
                &target_name
            );
        }
        AssertionType::FileContent(expected) => {
            assert!(status.success());
            let content = handle
                .read_to_string(target_name.as_str())
                .expect("read target file");
            assert_eq!(content.trim(), expected);
        }
    }
}

#[rstest]
fn errors_when_action_missing() {
    let mut graph = BuildGraph::default();
    let edge = BuildEdge {
        action_id: "missing".into(),
        inputs: Vec::new(),
        explicit_outputs: vec![Utf8PathBuf::from("out")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    };
    graph.targets.insert(Utf8PathBuf::from("out"), edge);
    let err = generate(&graph).expect_err("missing action");
    assert!(matches!(err, NinjaGenError::MissingAction { id } if id == "missing"));
}

#[rstest]
fn generate_format_error() {
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
    let err = generate_into(&graph, &mut writer).expect_err("format error");
    assert!(matches!(err, NinjaGenError::Format(_)));
}
