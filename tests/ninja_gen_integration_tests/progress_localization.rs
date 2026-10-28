//! Verify localized progress labels through Ninja generation.

use anyhow::{Result, ensure};
use camino::Utf8PathBuf;
use netsuke::ast::{CommandIsolation, Recipe, StringOrList};
use netsuke::ir::{Action, BuildEdge, BuildGraph, DependencyOrder};
use netsuke::manifest;
use netsuke::ninja_gen::generate;
use test_support::localizer::locale_localizer;

#[test]
fn german_inline_progress_uses_localized_scalar_and_list_counts() -> Result<()> {
    let _localizer = locale_localizer("de-DE");
    let manifest = manifest::from_str(concat!(
        "netsuke_version: '1.0.0'\n",
        "targets:\n",
        "  - name: scalar-check\n",
        "    command: echo scalar\n",
        "  - name: singleton-check\n",
        "    command:\n",
        "      - echo only\n",
        "  - name: multi-check\n",
        "    command:\n",
        "      - echo first\n",
        "      - echo second\n",
    ))?;
    let graph = BuildGraph::from_manifest(&manifest)?;
    let ninja = generate(&graph)?;
    let singular_list_label = concat!(
        "description = Wird gebaut: \u{2068}singleton-check\u{2069} ",
        "(\u{2068}1\u{2069} Befehl)"
    );
    let plural_list_label = concat!(
        "description = Wird gebaut: \u{2068}multi-check\u{2069} ",
        "(\u{2068}2\u{2069} Befehle)"
    );

    ensure!(
        ninja.contains("description = Wird gebaut: \u{2068}scalar-check\u{2069}")
            && ninja.contains(singular_list_label)
            && ninja.contains(plural_list_label)
            && !ninja.contains("description = Building "),
        "the German catalogue should supply generated progress text: {ninja}"
    );
    Ok(())
}

#[test]
fn hebrew_inline_command_lists_use_singular_and_plural_forms() -> Result<()> {
    let _localizer = locale_localizer("he");
    let manifest = manifest::from_str(concat!(
        "netsuke_version: '1.0.0'\n",
        "targets:\n",
        "  - name: singleton-check\n",
        "    command:\n",
        "      - echo only\n",
        "  - name: multi-check\n",
        "    command:\n",
        "      - echo first\n",
        "      - echo second\n",
    ))?;
    let graph = BuildGraph::from_manifest(&manifest)?;
    let ninja = generate(&graph)?;
    let singular_label = concat!(
        "description = \u{200f}בונה \u{2068}singleton-check\u{2069} ",
        "(\u{2068}1\u{2069} פקודה)"
    );
    let plural_label = concat!(
        "description = \u{200f}בונה \u{2068}multi-check\u{2069} ",
        "(\u{2068}2\u{2069} פקודות)"
    );

    ensure!(
        ninja.contains(singular_label)
            && ninja.contains(plural_label)
            && !ninja.contains("description = Building "),
        "the Hebrew catalogue should select singular and plural progress text: {ninja}"
    );
    Ok(())
}

#[test]
fn arabic_generation_preserves_fluent_isolates_and_sanitizes_input() -> Result<()> {
    let _localizer = locale_localizer("ar");
    let action = Action {
        recipe: Recipe::Command {
            command: StringOrList::String("echo ready".into()),
            isolation: CommandIsolation::Shared,
        },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    };
    let edge = BuildEdge {
        action_id: "rtl-progress".into(),
        inputs: Vec::new(),
        implicit_deps: Vec::new(),
        dependency_order: DependencyOrder::Parallel,
        explicit_outputs: vec![Utf8PathBuf::from("out")],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        progress_label: Some("left\u{202e}right".into()),
        phony: false,
        always: false,
    };
    let mut graph = BuildGraph::default();
    graph.actions.insert(edge.action_id.clone(), action);
    graph.insert_edge(edge)?;

    let ninja = generate(&graph)?;
    ensure!(
        ninja.contains("description = \u{200f}بناء \u{2068}left\\u{202e}right\u{2069}"),
        "Arabic progress should preserve Fluent isolation around visibly escaped input: {ninja}"
    );
    Ok(())
}
