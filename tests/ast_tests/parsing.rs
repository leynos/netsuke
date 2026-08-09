//! Tests for manifest deserialization: required and optional fields, rejection
//! of unknown or ill-shaped sections, and the target-level boolean flags.

use anyhow::{Context, Result, bail, ensure};
use netsuke::ast::{Recipe, StringOrList};
use netsuke::localization::{self, keys};
use rstest::rstest;
use semver::Version;
use test_support::display_error_chain;

use super::support::parse_manifest;

#[rstest]
fn parse_minimal_manifest() -> Result<()> {
    let yaml = r#"netsuke_version: "1.0.0"
targets:
  - name: hello
    command: "echo hi""#;

    let manifest = parse_manifest(yaml)?;
    let expected_version = Version::parse("1.0.0")?;
    ensure!(
        manifest.netsuke_version == expected_version,
        "unexpected manifest version: got {}, expected {}",
        manifest.netsuke_version,
        expected_version
    );
    let first = manifest
        .targets
        .first()
        .context("manifest should contain at least one target")?;
    let name = match &first.name {
        StringOrList::String(name) => name,
        other => bail!("Expected target name to be StringOrList::String, got: {other:?}"),
    };
    ensure!(name == "hello", "unexpected target name: {name}");

    if let Recipe::Command { command } = &first.recipe {
        ensure!(
            *command == StringOrList::String("echo hi".into()),
            "unexpected command: {command:?}"
        );
    } else {
        bail!("Expected command recipe, got: {:?}", first.recipe);
    }
    Ok(())
}

#[test]
fn missing_required_fields() -> Result<()> {
    {
        let yaml = r#"
            targets:
              - name: hello
                command: "echo hi"
        "#;
        ensure!(
            parse_manifest(yaml).is_err(),
            "manifest missing version should fail"
        );
    }

    {
        let yaml = r#"
            netsuke_version: "1.0.0"
        "#;
        ensure!(
            parse_manifest(yaml).is_err(),
            "manifest missing targets should fail"
        );
    }

    {
        let yaml = r#"
            netsuke_version: "1.0.0"
            targets:
              - command: "echo hi"
        "#;
        ensure!(
            parse_manifest(yaml).is_err(),
            "target missing name should fail"
        );
    }
    Ok(())
}

#[test]
fn unknown_fields() -> Result<()> {
    {
        let yaml = r#"
            netsuke_version: "1.0.0"
            targets:
              - name: hello
                command: "echo hi"
            extra: 42
        "#;
        ensure!(
            parse_manifest(yaml).is_err(),
            "manifest with extra top-level field should fail"
        );
    }

    {
        let yaml = r#"
            netsuke_version: "1.0.0"
            targets:
              - name: hello
                command: "echo hi"
                unexpected: true
        "#;
        ensure!(
            parse_manifest(yaml).is_err(),
            "manifest with unexpected target field should fail"
        );
    }
    Ok(())
}

#[test]
fn vars_section_must_be_object() -> Result<()> {
    let yaml = r#"
        netsuke_version: "1.0.0"
        vars:
          - not: mapping
        targets:
          - name: hello
            command: "echo hi"
    "#;
    let err = parse_manifest(yaml)
        .err()
        .context("vars should be an object")?;
    let chain = err
        .chain()
        .map(|e: &(dyn std::error::Error + '_)| e.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    let expected = localization::message(keys::MANIFEST_VARS_NOT_OBJECT).to_string();
    ensure!(
        chain.contains(&expected),
        "unexpected error message: {chain}"
    );
    Ok(())
}

/// A `vars` key matching a built-in helper would silently replace it, because
/// `MiniJinja` stores functions and globals in one namespace.
#[rstest]
#[case::env("env")]
#[case::glob("glob")]
fn vars_section_rejects_reserved_helper_names(#[case] reserved: &str) -> Result<()> {
    let yaml = format!(
        r#"
        netsuke_version: "1.0.0"
        vars:
          {reserved}: shadowed
          greeting: hi
        targets:
          - name: hello
            command: "echo hi"
    "#
    );
    let err = parse_manifest(&yaml)
        .err()
        .context("reserved vars key should be rejected")?;
    let chain = display_error_chain(err.as_ref());
    let expected = localization::message(keys::MANIFEST_VARS_RESERVED_NAME)
        .with_arg("name", reserved)
        .to_string();
    ensure!(
        chain.contains(&expected),
        "unexpected error message: {chain}"
    );
    Ok(())
}

/// Non-reserved variables must keep working alongside the built-in helpers.
#[rstest]
fn vars_section_allows_non_reserved_names() -> Result<()> {
    let yaml = r#"
        netsuke_version: "1.0.0"
        vars:
          greeting: hi
        targets:
          - name: hello
            command: "echo {{ greeting }}"
    "#;
    let manifest = parse_manifest(yaml)?;
    let first = manifest.targets.first().context("expected one target")?;
    let Recipe::Command { command } = &first.recipe else {
        bail!("expected a command recipe, got {:?}", first.recipe);
    };
    ensure!(
        *command == StringOrList::String("echo hi".into()),
        "unexpected command: {command:?}"
    );
    Ok(())
}

#[test]
fn empty_lists_and_maps() -> Result<()> {
    {
        let yaml = r#"
            netsuke_version: "1.0.0"
            targets: []
        "#;
        let manifest = parse_manifest(yaml)?;
        ensure!(
            manifest.targets.is_empty(),
            "expected no targets for empty list manifest"
        );
    }

    {
        let yaml = r#"
            netsuke_version: "1.0.0"
            targets:
              - name: hello
                command: {}
        "#;
        ensure!(
            parse_manifest(yaml).is_err(),
            "empty rule map should fail to parse"
        );
    }

    {
        let yaml = r#"
            netsuke_version: "1.0.0"
            targets:
              - name: hello
                script: {}
        "#;
        ensure!(
            parse_manifest(yaml).is_err(),
            "empty script map should fail to parse"
        );
    }

    {
        let yaml = r#"
            netsuke_version: "1.0.0"
            targets:
              - name: hello
                rule: {}
        "#;
        ensure!(
            parse_manifest(yaml).is_err(),
            "empty rule map should fail to parse"
        );
    }
    Ok(())
}

#[test]
fn optional_fields() -> Result<()> {
    {
        let yaml = r#"
            netsuke_version: "1.0.0"
            rules:
              - name: compile
                command: cc
                description: "Compile"
            targets:
              - name: hello
                rule: compile
        "#;
        let manifest = parse_manifest(yaml)?;
        let rule = manifest
            .rules
            .first()
            .context("expected at least one rule")?;
        ensure!(
            rule.description.as_deref() == Some("Compile"),
            "unexpected rule description: {:?}",
            rule.description
        );
    }

    {
        let yaml = r#"
            netsuke_version: "1.0.0"
            rules:
              - name: compile
                command: cc
            targets:
              - name: hello
                rule: compile
        "#;
        let manifest = parse_manifest(yaml)?;
        let rule = manifest
            .rules
            .first()
            .context("expected at least one rule")?;
        ensure!(rule.description.is_none(), "description should be absent");
    }
    Ok(())
}

#[test]
fn rule_level_deps_are_rejected() -> Result<()> {
    let yaml = r#"
        netsuke_version: "1.0.0"
        rules:
          - name: compile
            command: cc
            deps: generated/header.h
        targets:
          - name: hello
            rule: compile
    "#;
    let error = parse_manifest(yaml).expect_err("rule-level deps should be rejected");
    let message = display_error_chain(error.as_ref());
    let localized_summary = localization::message(keys::MANIFEST_PARSE).to_string();
    ensure!(
        message.starts_with(&localized_summary) && message.contains("unknown field `deps`"),
        "rule-level deps should produce a clear validation error: {message}"
    );
    Ok(())
}

#[rstest]
#[case::invalid_enum_variant(
    r#"
    netsuke_version: "1.0.0"
    targets:
      - name: hello
        kind: not_a_kind
        command: "echo hi"
"#
)]
#[case::actions_missing_recipe(
    r#"
    netsuke_version: "1.0.0"
    actions:
      - name: setup
    targets:
      - name: done
        command: "true"
"#
)]
fn parsing_failures(#[case] yaml: &str) {
    assert!(
        parse_manifest(yaml).is_err(),
        "manifest should fail: {yaml}"
    );
}

#[test]
fn phony_and_always_flags() -> Result<()> {
    {
        let yaml = r#"
            netsuke_version: "1.0.0"
            targets:
              - name: clean
                command: rm -rf build
                phony: true
                always: true
        "#;
        let manifest = parse_manifest(yaml)?;
        let target = manifest.targets.first().context("expected target entry")?;
        ensure!(target.phony, "target should be phony");
        ensure!(target.always, "target should always run");
    }

    {
        let yaml = r#"
            netsuke_version: "1.0.0"
            targets:
              - name: clean
                command: rm -rf build
        "#;
        let manifest = parse_manifest(yaml)?;
        let target = manifest.targets.first().context("expected target entry")?;
        ensure!(!target.phony, "target should not be phony");
        ensure!(!target.always, "target should not always run");
    }
    Ok(())
}
