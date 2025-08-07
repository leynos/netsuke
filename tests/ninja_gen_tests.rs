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
use rstest::rstest;
use std::{fs, path::PathBuf, process::Command};
use tempfile::tempdir;

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

#[rstest]
fn generate_phony() {
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
        inputs: vec![PathBuf::from("in")],
        explicit_outputs: vec![PathBuf::from("out")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: true,
        always: false,
    };
    let mut graph = BuildGraph::default();
    graph.actions.insert("a".into(), action);
    graph.targets.insert(PathBuf::from("out"), edge);

    let ninja = generate(&graph);
    let expected = concat!(
        "rule a\n",
        "  command = true\n\n",
        "build out: phony in\n\n",
    );
    assert_eq!(ninja, expected);
}

#[rstest]
fn generate_standard_build() {
    let action = Action {
        recipe: Recipe::Command {
            command: "cc -c $in -o $out".into(),
        },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    };
    let edge = BuildEdge {
        action_id: "compile".into(),
        inputs: vec![PathBuf::from("a.c"), PathBuf::from("b.c")],
        explicit_outputs: vec![PathBuf::from("ab.o")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    };
    let mut graph = BuildGraph::default();
    graph.actions.insert("compile".into(), action);
    graph.targets.insert(PathBuf::from("ab.o"), edge);

    let ninja = generate(&graph);
    let expected = concat!(
        "rule compile\n",
        "  command = cc -c $in -o $out\n\n",
        "build ab.o: compile a.c b.c\n\n",
    );
    assert_eq!(ninja, expected);
}

#[rstest]
fn generate_complex_dependencies() {
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
        action_id: "b".into(),
        inputs: vec![PathBuf::from("in")],
        explicit_outputs: vec![PathBuf::from("out"), PathBuf::from("log")],
        implicit_outputs: vec![PathBuf::from("out.d")],
        order_only_deps: vec![PathBuf::from("stamp")],
        phony: false,
        always: false,
    };
    let mut graph = BuildGraph::default();
    graph.actions.insert("b".into(), action);
    graph.targets.insert(PathBuf::from("out"), edge);

    let ninja = generate(&graph);
    let expected = concat!(
        "rule b\n",
        "  command = true\n\n",
        "build out log | out.d: b in || stamp\n\n",
    );
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
    let lines: Vec<&str> = ninja.lines().collect();
    let command_line = lines.get(1).expect("command line");
    assert!(
        command_line.contains("\\n"),
        "script newline should be encoded",
    );
}

/// Ensure a multi-line script produces a Ninja manifest that Ninja accepts.
#[rstest]
#[ignore = "requires Ninja"]
fn integration_multiline_script_valid() {
    if skip_if_ninja_unavailable() {
        return;
    }

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
    let dir = tempdir().expect("temp dir");
    fs::write(dir.path().join("build.ninja"), &ninja).expect("write ninja");
    let status = Command::new("ninja")
        .arg("-n")
        .current_dir(dir.path())
        .status()
        .expect("run ninja");
    assert!(status.success());
}

/// Test that scripts containing percent signs execute correctly.
#[rstest]
#[ignore = "requires Ninja"]
fn generate_script_with_percent() {
    if skip_if_ninja_unavailable() {
        return;
    }

    let action = Action {
        recipe: Recipe::Script {
            script: "echo 100% > out".into(),
        },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    };
    let edge = BuildEdge {
        action_id: "percent".into(),
        inputs: Vec::new(),
        explicit_outputs: vec![PathBuf::from("out")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    };
    let mut graph = BuildGraph::default();
    graph.actions.insert("percent".into(), action);
    graph.targets.insert(PathBuf::from("out"), edge);
    graph.default_targets.push(PathBuf::from("out"));

    let ninja = generate(&graph);
    let dir = tempdir().expect("temp dir");
    fs::write(dir.path().join("build.ninja"), &ninja).expect("write ninja");
    let status = Command::new("ninja")
        .arg("out")
        .current_dir(dir.path())
        .status()
        .expect("run ninja");
    assert!(status.success());
    let content = fs::read_to_string(dir.path().join("out")).expect("read out");
    assert_eq!(content.trim(), "100%");
}
