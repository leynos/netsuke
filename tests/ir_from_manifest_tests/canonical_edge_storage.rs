//! Structural tests for canonical multi-output build-edge storage.

use anyhow::{Context, Result, bail, ensure};
use camino::Utf8PathBuf;
use netsuke::{
    ir::{BuildGraph, IrGenError},
    manifest, ninja_gen,
};

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
        canonical.explicit_outputs.len() == OUTPUT_COUNT,
        "canonical edge should retain every explicit output"
    );
    ensure!(
        graph.output_count() == OUTPUT_COUNT,
        "every explicit output should remain an output alias"
    );
    ensure!(
        canonical.explicit_outputs.len() + graph.output_count() == OUTPUT_COUNT * 2,
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
