//! Tests for the opt-in Ninja console scheduling field.

use anyhow::{Context, Result, ensure};
use rstest::rstest;

use super::support::parse_manifest;
use netsuke::manifest::ManifestError;

/// Default target console scheduling to false and accept either explicit value.
#[rstest]
#[case::omitted("", false)]
#[case::explicit_false("console: false", false)]
#[case::explicit_true("console: true", true)]
fn target_console_defaults_and_overrides(
    #[case] console_field: &str,
    #[case] expected_console: bool,
) -> Result<()> {
    let yaml = format!(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: lint\n    command: 'cargo clippy'\n    {console_field}\n"
    );
    let manifest = parse_manifest(&yaml)?;
    let target = manifest.targets.first().context("expected target entry")?;
    ensure!(
        target.console == expected_console,
        "unexpected console flag: got {}, expected {}",
        target.console,
        expected_console
    );
    Ok(())
}

/// Preserve the parse diagnostic path while localizing dependency-only console errors.
#[test]
fn dependency_only_console_manifest_error_is_localized() -> Result<()> {
    let _locale = test_support::localizer::locale_localizer("fr");
    let yaml = r#"
        netsuke_version: "1.0.0"
        targets:
          - name: all
            deps: [lint]
            console: true
    "#;
    let error = parse_manifest(yaml).expect_err("dependency-only console must be rejected");
    ensure!(
        matches!(
            error.downcast_ref::<ManifestError>(),
            Some(ManifestError::Parse { .. })
        ),
        "manifest validation should retain the parse diagnostic path: {error:#}"
    );
    let rendered = format!("{error:#}");
    ensure!(
        rendered.contains("arête de compilation"),
        "dependency-only console diagnostic should use the French catalogue: {rendered}"
    );
    Ok(())
}

/// Keep contention fields unknown until RFC 0024 defines a manifest schema.
#[test]
fn contention_remains_an_unknown_manifest_field() {
    let yaml = r#"
        netsuke_version: "1.0.0"
        targets:
          - name: lint
            command: "cargo clippy"
            console: true
            contention: developer-gates
    "#;
    assert!(
        parse_manifest(yaml).is_err(),
        "contention should remain an unknown manifest field"
    );
}
