//! Manifest lowering contracts for edge-local inline progress labels.

use anyhow::{Context, Result, ensure};
use netsuke::{
    ast::Recipe,
    hasher::ActionHasher,
    ir::BuildGraph,
    manifest,
    ninja_gen::{self, RecipeShell},
};
use rstest::rstest;
use test_support::{localizer_test_lock, set_en_localizer, sha256_hex};

/// Retain the first declared output for every executable inline recipe shape.
#[rstest]
#[case::scalar("command: echo built\n")]
#[case::command_list("command:\n      - echo first\n      - echo second\n")]
#[case::script("script: |\n      echo built\n")]
fn inline_recipes_store_the_raw_output_name(#[case] recipe: &str) -> Result<()> {
    let manifest = manifest::from_str(&format!(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: check-fmt\n    {recipe}"
    ))?;
    let graph = BuildGraph::from_manifest(&manifest).context("lower inline recipe")?;
    let edge = graph.edges().next().context("expected one inline edge")?;

    ensure!(
        edge.progress_label.as_deref() == Some("check-fmt"),
        "store the raw first output name: {:?}",
        edge.progress_label
    );
    Ok(())
}

/// Keep named-rule progress authoritative and leave inline provenance absent.
#[rstest]
#[case::without_description(None)]
#[case::with_description(Some("Rule progress"))]
fn named_rules_never_retain_inline_progress_labels(
    #[case] rule_description: Option<&str>,
) -> Result<()> {
    let rule_description_yaml =
        rule_description.map_or_else(String::new, |value| format!("    description: {value}\n"));
    let manifest = manifest::from_str(&format!(
        concat!(
            "netsuke_version: '1.0.0'\n",
            "rules:\n",
            "  - name: compile\n",
            "{description}",
            "    command: echo compiled\n",
            "targets:\n",
            "  - name: out/app\n",
            "    description: Discovery only\n",
            "    rule: compile\n",
        ),
        description = rule_description_yaml
    ))?;
    let graph = BuildGraph::from_manifest(&manifest).context("lower named rule")?;
    let edge = graph
        .edges()
        .next()
        .context("expected one named-rule edge")?;
    let action = graph
        .actions
        .get(&edge.action_id)
        .context("expected named-rule action")?;
    let ninja = ninja_gen::generate_with_shell(&graph, RecipeShell::Posix)?;

    ensure!(
        edge.progress_label.is_none(),
        "named rules have no inline label"
    );
    ensure!(
        action.description.as_deref() == rule_description,
        "preserve the named-rule description"
    );
    ensure!(
        !ninja.contains("Discovery only"),
        "target descriptions remain discovery metadata: {ninja}"
    );
    if let Some(expected_description) = rule_description {
        ensure!(
            ninja.contains(&format!("description = {expected_description}")),
            "named-rule progress description should remain authoritative: {ninja}"
        );
    } else {
        ensure!(
            !ninja.contains("description ="),
            "a rule without description must not acquire an inline label: {ninja}"
        );
    }
    Ok(())
}

/// Preserve action identity and command-list failure fingerprints across names.
#[test]
fn deduplicated_inline_edges_keep_distinct_labels_and_shared_failure_identity() -> Result<()> {
    let _lock = localizer_test_lock().map_err(|error| anyhow::anyhow!("{error}"))?;
    let _localizer = set_en_localizer();
    let manifest = manifest::from_str(concat!(
        "netsuke_version: '1.0.0'\n",
        "targets:\n",
        "  - name: check-fmt\n",
        "    command:\n",
        "      - echo first\n",
        "      - echo second\n",
        "  - name: lint\n",
        "    command:\n",
        "      - echo first\n",
        "      - echo second\n",
    ))?;
    let graph = BuildGraph::from_manifest_for_shell(&manifest, RecipeShell::Posix)?;
    let edges = graph.edges().collect::<Vec<_>>();
    ensure!(edges.len() == 2, "expected two inline edges");
    let first_edge = edges.first().context("expected first inline edge")?;
    let second_edge = edges.get(1).context("expected second inline edge")?;
    ensure!(
        first_edge.action_id == second_edge.action_id,
        "identical recipes must still share one action identity"
    );
    ensure!(
        graph.actions.len() == 1,
        "deduplication must retain one canonical action"
    );

    let action = graph
        .actions
        .get(&first_edge.action_id)
        .context("expected canonical action")?;
    ensure!(
        ActionHasher::hash(action)? == first_edge.action_id,
        "edge-local labels must not enter the action hash"
    );
    let Recipe::Command { command, .. } = &action.recipe else {
        anyhow::bail!("expected a command-list action")
    };
    ensure!(
        command.to_string_vec() == ["echo first", "echo second"],
        "deduplicated action command text must remain unchanged: {command:?}"
    );

    let ninja = ninja_gen::generate_with_shell(&graph, RecipeShell::Posix)?;
    ensure!(
        ninja.contains("description = Building \u{2068}check-fmt\u{2069}")
            && ninja.contains("description = Building \u{2068}lint\u{2069}"),
        "each edge should retain its own readable label: {ninja}"
    );
    let identity = sha256_hex(first_edge.action_id.as_bytes());
    ensure!(
        ninja.contains(&format!(
            "netsuke command-list failure: action {identity}, entry 1"
        )),
        "the shared command must retain the fingerprint of its canonical action: {ninja}"
    );
    ensure!(
        ninja.matches("command = ").count() == 1,
        "the shared action should emit one unchanged command binding: {ninja}"
    );
    Ok(())
}
