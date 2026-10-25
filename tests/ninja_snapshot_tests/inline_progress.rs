//! Snapshot readable inline progress and its serial-edge rendering.

use anyhow::{Context, Result, ensure};
use insta::{Settings, assert_snapshot};
use netsuke::{ir::BuildGraph, manifest, ninja_gen, ninja_gen::RecipeShell};
use test_support::{localizer_test_lock, set_en_localizer};

/// Snapshot scalar, list, script and named-rule progress in one manifest.
#[test]
fn combined_manifest_uses_inline_progress_and_preserves_rule_descriptions() -> Result<()> {
    let _lock = localizer_test_lock().map_err(|error| anyhow::anyhow!("{error}"))?;
    let _localizer = set_en_localizer();
    let manifest = manifest::from_str(concat!(
        "netsuke_version: '1.0.0'\n",
        "rules:\n",
        "  - name: named-rule\n",
        "    description: Named rule wording\n",
        "    command: echo named\n",
        "actions:\n",
        "  - name: check-fmt\n",
        "    description: Discovery metadata only\n",
        "    command: echo scalar\n",
        "  - name: lint\n",
        "    command:\n",
        "      - echo first\n",
        "      - echo second\n",
        "targets:\n",
        "  - name: out/script\n",
        "    script: |\n",
        "      echo scripted\n",
        "  - name: out/named\n",
        "    rule: named-rule\n",
    ))?;
    let graph = BuildGraph::from_manifest(&manifest).context("lower combined manifest")?;
    let ninja = ninja_gen::generate_with_shell(&graph, RecipeShell::Posix)?;

    ensure!(
        ninja.contains("description = Building check-fmt")
            && ninja.contains("description = Building lint (2 commands)")
            && ninja.contains("description = Building out/script")
            && ninja.contains("description = Named rule wording")
            && !ninja.contains("Discovery metadata only"),
        "combined output should localize inline progress without using discovery metadata: {ninja}"
    );

    let mut settings = Settings::new();
    settings.set_snapshot_path(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/snapshots/ninja"
    ));
    settings.bind(|| {
        assert_snapshot!("inline_progress_combined_manifest_ninja", ninja);
    });
    Ok(())
}

/// Keep localized progress on a serial multi-output edge in the bundle writer.
#[test]
fn serial_multi_output_edge_keeps_first_output_progress_description() -> Result<()> {
    let _lock = localizer_test_lock().map_err(|error| anyhow::anyhow!("{error}"))?;
    let _localizer = set_en_localizer();
    let manifest = manifest::from_str(concat!(
        "netsuke_version: '1.0.0'\n",
        "targets:\n",
        "  - name: [check-fmt, check-fmt.stamp]\n",
        "    deps: [prepare, validate]\n",
        "    dependency_order: serial\n",
        "    command: echo ready\n",
    ))?;
    let graph = BuildGraph::from_manifest(&manifest).context("lower serial manifest")?;
    let bundle = ninja_gen::generate_bundle(&graph)?;
    let build_file = bundle.build_file();
    let edge = build_file
        .split("\n\n")
        .find(|block| block.starts_with("build check-fmt check-fmt.stamp:"))
        .context("expected one canonical serial build edge")?;

    ensure!(
        edge.contains("description = Building check-fmt"),
        "serial multi-output edge should use its first output label: {edge}"
    );
    ensure!(
        build_file.contains("build .netsuke/serial/") && bundle.dyndep_files().len() == 2,
        "serial edge should retain its dyndep gates and sidecars: {build_file}"
    );
    Ok(())
}
