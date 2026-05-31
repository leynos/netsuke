//! Unit tests for [`netsuke::ninja_gen`] Ninja file generation.
//!
//! Exercises [`netsuke::ninja_gen::generate`] by constructing
//! [`netsuke::ir::BuildGraph`] values directly and asserting the emitted Ninja
//! syntax. Snapshot tests use `insta` to guard output stability.

use anyhow::{Result, ensure};
use camino::Utf8PathBuf;
use insta::{Settings, assert_snapshot};
use netsuke::ast::Recipe;
use netsuke::ir::{Action, BuildEdge, BuildGraph};
use netsuke::ninja_gen::generate;
use rstest::rstest;

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
        implicit_deps: Vec::new(),
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
        implicit_deps: Vec::new(),
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
        implicit_deps: Vec::new(),
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
#[case::explicit_inputs_plus_implicit_deps(
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
        implicit_deps: vec![Utf8PathBuf::from("dep")],
        explicit_outputs: vec![Utf8PathBuf::from("out")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    },
    Utf8PathBuf::from("out"),
    concat!(
        "rule b\n",
        "  command = true\n\n",
        "build out: b in | dep\n\n",
    ),
)]
#[case::implicit_deps_without_explicit_inputs(
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
        inputs: Vec::new(),
        implicit_deps: vec![Utf8PathBuf::from("dep")],
        explicit_outputs: vec![Utf8PathBuf::from("out")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
    },
    Utf8PathBuf::from("out"),
    concat!(
        "rule b\n",
        "  command = true\n\n",
        "build out: b | dep\n\n",
    ),
)]
#[case::all_dependency_classes(
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
        implicit_deps: vec![Utf8PathBuf::from("dep")],
        explicit_outputs: vec![Utf8PathBuf::from("out")],
        implicit_outputs: Vec::new(),
        order_only_deps: vec![Utf8PathBuf::from("stamp")],
        phony: false,
        always: false,
    },
    Utf8PathBuf::from("out"),
    concat!(
        "rule b\n",
        "  command = true\n\n",
        "build out: b in | dep || stamp\n\n",
    ),
)]
#[case::phony_action_with_implicit_deps(
    Action {
        recipe: Recipe::Command { command: "true".into() },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    },
    BuildEdge {
        action_id: "phony".into(),
        inputs: Vec::new(),
        implicit_deps: vec![Utf8PathBuf::from("dep")],
        explicit_outputs: vec![Utf8PathBuf::from("phony_action")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: true,
        always: false,
    },
    Utf8PathBuf::from("phony_action"),
    concat!(
        "rule phony\n",
        "  command = true\n\n",
        "build phony_action: phony | dep\n\n",
    ),
)]
fn generate_ninja_scenarios(
    #[case] action: Action,
    #[case] edge: BuildEdge,
    #[case] target_path: Utf8PathBuf,
    #[case] expected: &str,
) -> Result<()> {
    let mut graph = BuildGraph::default();
    graph.actions.insert(edge.action_id.clone(), action);
    graph.targets.insert(target_path, edge);

    let ninja = generate(&graph)?;
    ensure!(
        ninja == expected,
        "generated ninja manifest did not match expectation"
    );
    Ok(())
}

#[rstest]
fn generate_empty_graph() -> Result<()> {
    let graph = BuildGraph::default();
    let ninja = generate(&graph)?;
    ensure!(ninja.is_empty(), "expected empty ninja manifest");
    Ok(())
}

#[rstest]
fn generate_multiline_script_snapshot() -> Result<()> {
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
            implicit_deps: Vec::new(),
            explicit_outputs: vec![Utf8PathBuf::from("out")],
            implicit_outputs: Vec::new(),
            order_only_deps: Vec::new(),
            phony: false,
            always: false,
        },
    );
    graph.default_targets.push(Utf8PathBuf::from("out"));

    let ninja = generate(&graph)?;
    let mut settings = Settings::new();
    settings.set_snapshot_path(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/snapshots/ninja",
    ));
    settings.bind(|| {
        assert_snapshot!("multiline_script_ninja", &ninja);
    });
    ensure!(
        ninja.contains("printf %b") && ninja.contains("\\n"),
        "script should use printf %b with encoded newlines"
    );
    Ok(())
}
