//! Unit tests for Ninja file generation.
//!
//! Snapshot tests use `insta` to ensure the emitted manifest remains stable.
//! Tests cover various scenarios including phony targets, standard builds with
//! multiple inputs and outputs, complex dependency relationships, and edge
//! cases like empty build graphs.

use insta::{Settings, assert_snapshot};
use netsuke::ast::Recipe;
use netsuke::ir::{Action, BuildEdge, BuildGraph};
use netsuke::ninja_gen::generate;
use rstest::{fixture, rstest};
use std::{fs, path::PathBuf, process::Command};
use tempfile::{TempDir, tempdir};

fn skip_if_ninja_unavailable() -> bool {
    match Command::new("ninja").arg("--version").output() {
        Err(_) => {
            eprintln!("skipping test: ninja not found in PATH");
            true
        }
        Ok(output) if !output.status.success() => {
            eprintln!("skipping test: ninja --version failed");
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
        inputs: vec![PathBuf::from("in")],
        explicit_outputs: vec![PathBuf::from("out")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: true,
        always: false,
    },
    PathBuf::from("out"),
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
        inputs: vec![PathBuf::from("a.c"), PathBuf::from("b.c")],
        explicit_outputs: vec![PathBuf::from("ab.o")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    },
    PathBuf::from("ab.o"),
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
        inputs: vec![PathBuf::from("in")],
        explicit_outputs: vec![PathBuf::from("out"), PathBuf::from("log")],
        implicit_outputs: vec![PathBuf::from("out.d")],
        order_only_deps: vec![PathBuf::from("stamp")],
        phony: false,
        always: false,
    },
    PathBuf::from("out"),
    concat!(
        "rule b\n",
        "  command = true\n\n",
        "build out log | out.d: b in || stamp\n\n",
    ),
)]
fn generate_ninja_scenarios(
    #[case] action: Action,
    #[case] edge: BuildEdge,
    #[case] target_path: PathBuf,
    #[case] expected: &str,
) {
    let mut graph = BuildGraph::default();
    graph.actions.insert(edge.action_id.clone(), action);
    graph.targets.insert(target_path, edge);

    let ninja = generate(&graph);
    assert_eq!(ninja, expected);
}

#[rstest]
fn generate_empty_graph() {
    let graph = BuildGraph::default();
    let ninja = generate(&graph);
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
        PathBuf::from("out"),
        BuildEdge {
            action_id: "script".into(),
            inputs: Vec::new(),
            explicit_outputs: vec![PathBuf::from("out")],
            implicit_outputs: Vec::new(),
            order_only_deps: Vec::new(),
            phony: false,
            always: false,
        },
    );
    graph.default_targets.push(PathBuf::from("out"));

    let ninja = generate(&graph);
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
        explicit_outputs: vec![PathBuf::from("out")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    },
    PathBuf::from("out"),
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
        explicit_outputs: vec![PathBuf::from("out")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    },
    PathBuf::from("out"),
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
        explicit_outputs: vec![PathBuf::from("out")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    },
    PathBuf::from("out"),
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
        explicit_outputs: vec![PathBuf::from("say-hello")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: true,
        always: false,
    },
    PathBuf::from("action-called.txt"),
    vec!["say-hello"],
    AssertionType::FileExists,
)]
fn ninja_integration_tests(
    ninja_integration_setup: Option<TempDir>,
    #[case] action: Action,
    #[case] edge: BuildEdge,
    #[case] target_name: PathBuf,
    #[case] ninja_args: Vec<&str>,
    #[case] assertion: AssertionType,
) {
    let Some(dir) = ninja_integration_setup else {
        return;
    };

    let output = edge
        .explicit_outputs
        .first()
        .expect("explicit output")
        .clone();
    let mut graph = BuildGraph::default();
    graph.actions.insert(edge.action_id.clone(), action);
    graph.targets.insert(output.clone(), edge);
    graph.default_targets.push(output);

    let ninja = generate(&graph);
    fs::write(dir.path().join("build.ninja"), &ninja).expect("write ninja");
    let status = Command::new("ninja")
        .args(&ninja_args)
        .current_dir(dir.path())
        .status()
        .expect("run ninja");

    match assertion {
        AssertionType::StatusSuccess => assert!(status.success()),
        AssertionType::FileExists => {
            assert!(status.success());
            assert!(dir.path().join(target_name).exists());
        }
        AssertionType::FileContent(expected) => {
            assert!(status.success());
            let content =
                fs::read_to_string(dir.path().join(target_name)).expect("read target file");
            assert_eq!(content.trim(), expected);
        }
    }
}
