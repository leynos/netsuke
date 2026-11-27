//! Structural tests for canonical multi-output build-edge storage.

use anyhow::{Context, Result, bail, ensure};
use camino::Utf8PathBuf;
use netsuke::{
    ir::{BuildGraph, IrGenError},
    manifest, ninja_gen,
};
use test_support::{localizer_test_lock, set_en_localizer};

/// Verify that every output alias resolves to one canonical edge.
fn assert_canonical_output_aliases(
    graph: &BuildGraph,
    output_count: usize,
) -> Result<&netsuke::ir::BuildEdge> {
    let canonical = graph.edges().next().context("expected canonical edge")?;
    let first_output = canonical
        .explicit_outputs
        .first()
        .context("expected canonical output")?;
    let canonical_edge_id = graph
        .edge_id_for_output(first_output.as_path())
        .context("expected canonical edge identity")?;

    ensure!(graph.edge_count() == 1, "one target must own one edge");
    ensure!(
        canonical.explicit_outputs.len() == output_count,
        "canonical edge should retain every explicit output"
    );
    ensure!(
        graph.output_count() == output_count,
        "every explicit output should remain an output alias"
    );
    ensure!(
        canonical.explicit_outputs.len() + graph.output_count() == output_count * 2,
        "the edge vector and output index must grow linearly"
    );

    for output in &canonical.explicit_outputs {
        ensure!(
            graph.edge_id_for_output(output.as_path()) == Some(canonical_edge_id),
            "every output must resolve to the canonical edge identity"
        );
        let (_, resolved) = graph
            .target_for_output(output.as_path())
            .with_context(|| format!("expected producer for {output}"))?;
        ensure!(
            std::ptr::eq(canonical, resolved),
            "every output must resolve to the canonical edge"
        );
    }

    Ok(canonical)
}

/// Store one multi-output target in one canonical edge and many output aliases.
#[test]
fn multi_output_target_uses_linear_canonical_storage() -> Result<()> {
    const OUTPUT_COUNT: usize = 4_096;
    let outputs = (0..OUTPUT_COUNT)
        .map(|index| format!("out/{index:04}"))
        .collect::<Vec<_>>()
        .join(", ");
    let manifest = manifest::from_str(&format!(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: [{outputs}]\n    command: echo {{{{ outs }}}}\n"
    ))?;
    let graph = BuildGraph::from_manifest(&manifest)?;
    let canonical = assert_canonical_output_aliases(&graph, OUTPUT_COUNT)?;

    let ninja = ninja_gen::generate(&graph)?;
    let build_statements = ninja
        .lines()
        .filter(|line| line.starts_with("build "))
        .collect::<Vec<_>>();
    ensure!(
        build_statements.len() == 1,
        "one canonical edge must emit one Ninja build statement"
    );
    let statement = build_statements
        .first()
        .context("expected Ninja build statement")?;
    let generated_outputs = statement
        .strip_prefix("build ")
        .and_then(|line| line.split_once(':'))
        .map(|(output_list, _)| output_list.split_ascii_whitespace().collect::<Vec<_>>())
        .context("expected Ninja build statement outputs before ':'")?;
    let expected_outputs = canonical
        .explicit_outputs
        .iter()
        .map(|output| output.as_str())
        .collect::<Vec<_>>();
    ensure!(
        generated_outputs == expected_outputs,
        "the Ninja build statement must list every explicit output in order"
    );
    Ok(())
}

/// Keep progress provenance on one canonical edge using its first output.
#[test]
fn inline_multi_output_progress_uses_first_declared_name() -> Result<()> {
    let _lock = localizer_test_lock().map_err(|error| anyhow::anyhow!("{error}"))?;
    let _localizer = set_en_localizer();
    let manifest = manifest::from_str(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: [first.out, second.out]\n    command: echo built\n",
    )?;
    let graph = BuildGraph::from_manifest(&manifest)?;
    let canonical = assert_canonical_output_aliases(&graph, 2)?;
    let ninja = ninja_gen::generate(&graph)?;

    ensure!(
        canonical.progress_label.as_deref() == Some("first.out"),
        "the edge label should use the first declared output: {:?}",
        canonical.progress_label
    );
    ensure!(
        ninja.contains("description = Building \u{2068}first.out\u{2069}")
            && !ninja.contains("Building \u{2068}second.out\u{2069}"),
        "the canonical edge should emit one first-output label: {ninja}"
    );
    Ok(())
}

/// Resolve a dependency that names a non-first output alias of one edge.
///
/// Cycle detection and missing-dependency reporting both resolve a dependency
/// path through the output index. Indexing only the first alias would mistake
/// `second` for an external file, so the manifest below would lower cleanly
/// instead of reporting the cycle that runs through it.
#[test]
fn non_first_output_alias_resolves_for_cycle_detection() -> Result<()> {
    let manifest = manifest::from_str(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: [first, second]\n    sources: downstream\n    command: echo\n  - name: downstream\n    sources: second\n    command: echo\n",
    )?;
    let error = BuildGraph::from_manifest(&manifest)
        .err()
        .context("a cycle through the second alias must fail lowering")?;
    let IrGenError::CircularDependency { cycle, .. } = error else {
        bail!("expected a circular dependency, got {error}");
    };
    ensure!(
        cycle.contains(&Utf8PathBuf::from("second")),
        "the reported cycle must name the second alias: {cycle:?}"
    );
    Ok(())
}

/// Number of output aliases and external sources in the alias-budget fixtures.
const MANY_ALIAS_COUNT: usize = 64;

/// Render the comma-separated aliases or external sources for a fixture.
fn numbered_names(prefix: &str) -> String {
    (0..MANY_ALIAS_COUNT)
        .map(|index| format!("{prefix}{index:02}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Render a manifest whose one target owns many aliases and external sources.
///
/// `extra_source` closes a cycle through the second alias when present.
fn many_alias_manifest(extra_source: Option<&str>) -> String {
    let aliases = numbered_names("out");
    let mut sources = numbered_names("external");
    if let Some(source) = extra_source {
        sources.push_str(", ");
        sources.push_str(source);
    }
    let mut manifest = format!(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: [{aliases}]\n    sources: [{sources}]\n    command: echo\n"
    );
    if extra_source.is_some() {
        manifest.push_str("  - name: downstream\n    sources: out01\n    command: echo\n");
    }
    manifest
}

/// Lower one target with many aliases and absent sources to one canonical edge.
#[test]
fn many_alias_target_lowers_to_one_canonical_edge() -> Result<()> {
    let manifest = manifest::from_str(&many_alias_manifest(None))?;
    let graph = BuildGraph::from_manifest(&manifest)
        .context("a target with many aliases and external sources must lower")?;

    assert_canonical_output_aliases(&graph, MANY_ALIAS_COUNT)?;
    ensure!(
        graph.edge_count() == 1,
        "many aliases must still own one canonical edge",
    );
    Ok(())
}

/// A cycle closed through a non-first alias reports bounded missing records.
#[test]
fn many_alias_cycle_reports_one_record_per_external_source() -> Result<()> {
    let manifest = manifest::from_str(&many_alias_manifest(Some("downstream")))?;
    let error = BuildGraph::from_manifest(&manifest)
        .err()
        .context("a cycle through the second alias must fail lowering")?;
    let IrGenError::CircularDependency {
        cycle,
        missing_dependencies,
        ..
    } = error
    else {
        bail!("expected a circular dependency, got {error}");
    };

    ensure!(
        missing_dependencies.len() == MANY_ALIAS_COUNT,
        "one record per external source is expected, got {}: {:?}",
        missing_dependencies.len(),
        missing_dependencies,
    );
    let entry_alias = Utf8PathBuf::from("out01");
    ensure!(
        missing_dependencies
            .iter()
            .all(|(alias, _)| alias == &entry_alias),
        "every record must name the alias that expanded the edge: {missing_dependencies:?}",
    );
    ensure!(
        cycle.contains(&entry_alias),
        "the witness must name the alias the second target requested: {cycle:?}",
    );
    Ok(())
}
