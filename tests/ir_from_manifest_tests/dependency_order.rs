//! Dependency-order lowering tests for manifest actions and targets.

use anyhow::{Context, Result, ensure};
use camino::Utf8PathBuf;
use netsuke::{
    ir::{BuildGraph, DependencyOrder},
    manifest,
};
use rstest::rstest;

#[rstest]
#[case::target_serial(concat!(
    "netsuke_version: '1.0.0'\n",
    "targets:\n",
    "  - name: all\n",
    "    dependency_order: serial\n",
    "    deps: [check-fmt, lint, test]\n",
    "    command: echo $out\n",
), "all", false, &["check-fmt", "lint", "test"])]
#[case::action_serial(concat!(
    "netsuke_version: '1.0.0'\n",
    "actions:\n",
    "  - name: gate\n",
    "    dependency_order: serial\n",
    "    deps: [fmt, clippy]\n",
    "    command: echo $out\n",
    "targets: []\n",
), "gate", true, &["fmt", "clippy"])]
fn serial_dependency_order_survives_lowering(
    #[case] yaml: &str,
    #[case] output: &str,
    #[case] expected_phony: bool,
    #[case] expected_dependencies: &[&str],
) -> Result<()> {
    let manifest = manifest::from_str(yaml)?;
    let graph = BuildGraph::from_manifest(&manifest).context("expected graph generation")?;
    let edge = graph
        .targets
        .get(&Utf8PathBuf::from(output))
        .with_context(|| format!("expected edge for {output}"))?;
    ensure!(
        edge.dependency_order == DependencyOrder::Serial,
        "expected serial dependency order for {output}, got {:?}",
        edge.dependency_order
    );
    ensure!(
        edge.phony == expected_phony,
        "unexpected phony flag for {output}: {}",
        edge.phony
    );
    ensure!(
        edge.implicit_deps
            == expected_dependencies
                .iter()
                .map(Utf8PathBuf::from)
                .collect::<Vec<_>>(),
        "unexpected dependencies for {output}: {:?}",
        edge.implicit_deps
    );
    Ok(())
}

#[rstest]
fn parallel_dependency_order_lowering_is_default() -> Result<()> {
    let yaml = concat!(
        "netsuke_version: '1.0.0'\n",
        "targets:\n",
        "  - name: all\n",
        "    deps: [check-fmt, lint]\n",
        "    command: echo $out\n",
    );
    let manifest = manifest::from_str(yaml)?;
    let graph = BuildGraph::from_manifest(&manifest).context("expected graph generation")?;
    let edge = graph
        .targets
        .get(&Utf8PathBuf::from("all"))
        .context("expected edge for all")?;
    ensure!(
        edge.dependency_order == DependencyOrder::Parallel,
        "omission should default to parallel, got {:?}",
        edge.dependency_order
    );
    ensure!(
        edge.implicit_deps == vec![Utf8PathBuf::from("check-fmt"), Utf8PathBuf::from("lint")],
        "declaration order must be preserved through lowering: {:?}",
        edge.implicit_deps
    );
    Ok(())
}
