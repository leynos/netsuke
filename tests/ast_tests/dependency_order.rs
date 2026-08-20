//! Tests for the manifest `dependency_order` field.

use anyhow::{Context, Result, ensure};
use netsuke::ast::DependencyOrder;
use rstest::rstest;

use super::support::parse_manifest;

#[rstest]
fn omission_defaults_to_parallel() -> Result<()> {
    let manifest = parse_manifest(
        r#"
        netsuke_version: "1.0.0"
        targets:
          - name: all
            command: echo done
            deps: [check-fmt, test]
        "#,
    )?;
    let target = manifest.targets.first().context("expected target entry")?;
    ensure!(target.dependency_order == DependencyOrder::Parallel);
    Ok(())
}

#[rstest]
#[case::parallel("parallel", DependencyOrder::Parallel)]
#[case::serial("serial", DependencyOrder::Serial)]
fn explicit_value_parses(#[case] value: &str, #[case] expected: DependencyOrder) -> Result<()> {
    let manifest = parse_manifest(&format!(
        "netsuke_version: \"1.0.0\"\ntargets:\n  - name: all\n    command: echo done\n    dependency_order: {value}\n"
    ))?;
    let target = manifest.targets.first().context("expected target entry")?;
    ensure!(target.dependency_order == expected);
    Ok(())
}

#[rstest]
#[case::target("targets:\n  - name: all\n    command: echo done\n    dependency_order: sequential")]
#[case::action(
    "actions:\n  - name: setup\n    command: echo hi\n    dependency_order: sequential\ntargets:\n  - name: done\n    command: echo done"
)]
fn unknown_value_is_rejected(#[case] entities: &str) -> Result<()> {
    let yaml = format!("netsuke_version: \"1.0.0\"\n{entities}\n");
    ensure!(parse_manifest(&yaml).is_err());
    Ok(())
}
