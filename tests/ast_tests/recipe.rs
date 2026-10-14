//! Tests for recipe deserialization: the scalar and list forms of `command`,
//! and the rejection of an empty command list.

use anyhow::{Context, Result, bail, ensure};
use netsuke::ast::{CommandIsolation, Recipe, StringOrList};
use netsuke::localization::{self, keys};
use test_support::display_error_chain;

use super::support::parse_manifest;

#[test]
fn command_accepts_scalar_and_list_forms() -> Result<()> {
    {
        let yaml = r#"
            netsuke_version: "1.0.0"
            rules:
              - name: lint
                command: cargo clippy
            targets:
              - name: hello
                rule: lint
        "#;
        let manifest = parse_manifest(yaml)?;
        let rule = manifest.rules.first().context("expected one rule")?;
        let Recipe::Command { command, .. } = &rule.recipe else {
            bail!("expected command recipe, got {:?}", rule.recipe);
        };
        ensure!(
            command == &StringOrList::String("cargo clippy".into()),
            "unexpected scalar command: {command:?}"
        );
    }

    {
        let yaml = r#"
            netsuke_version: "1.0.0"
            rules:
              - name: comprehensive-check
                command:
                  - cargo fmt
                  - cargo clippy
                  - cargo test
            targets:
              - name: hello
                rule: comprehensive-check
        "#;
        let manifest = parse_manifest(yaml)?;
        let rule = manifest.rules.first().context("expected one rule")?;
        let Recipe::Command { command, .. } = &rule.recipe else {
            bail!("expected command recipe, got {:?}", rule.recipe);
        };
        ensure!(
            command
                == &StringOrList::List(
                    ["cargo fmt", "cargo clippy", "cargo test"]
                        .map(str::to_owned)
                        .to_vec()
                ),
            "unexpected list command: {command:?}"
        );
    }
    Ok(())
}

#[test]
fn command_list_isolation_defaults_to_shared() -> Result<()> {
    let yaml = r#"
        netsuke_version: "1.0.0"
        targets:
          - name: lint
            command:
              - cargo fmt
              - cargo clippy
    "#;
    let manifest = parse_manifest(yaml)?;
    let target = manifest.targets.first().context("expected one target")?;
    let Recipe::Command {
        command, isolation, ..
    } = &target.recipe
    else {
        bail!("expected command recipe, got {:?}", target.recipe);
    };
    ensure!(matches!(command, StringOrList::List(_)));
    ensure!(*isolation == CommandIsolation::Shared);
    let serialised = serde_json::to_value(&manifest)?;
    let serialized_target = serialised
        .get("targets")
        .and_then(serde_json::Value::as_array)
        .and_then(|targets| targets.first())
        .context("serialized manifest should contain one target")?;
    let serialized_command = serialized_target
        .get("Command")
        .and_then(serde_json::Value::as_object)
        .context("serialized target should contain its command recipe")?;
    ensure!(
        serialized_command.get("command_isolation").is_none(),
        "shared isolation must be omitted from manifest serialisation: {serialised}"
    );
    Ok(())
}

#[rstest::rstest]
#[case::explicit_shared("shared", CommandIsolation::Shared)]
#[case::isolated("isolated", CommandIsolation::Isolated)]
fn command_lists_accept_literal_isolation_modes(
    #[case] mode: &str,
    #[case] expected: CommandIsolation,
) -> Result<()> {
    let yaml = format!(
        "netsuke_version: 1.0.0\ntargets:\n  - name: lint\n    command:\n      - cargo fmt\n    command_isolation: {mode}\n"
    );
    let manifest = parse_manifest(&yaml)?;
    let target = manifest.targets.first().context("expected one target")?;
    let Recipe::Command { isolation, .. } = &target.recipe else {
        bail!("expected command recipe, got {:?}", target.recipe);
    };
    ensure!(
        *isolation == expected,
        "unexpected isolation mode: {isolation:?}"
    );
    let serialised = serde_json::to_value(&manifest)?;
    let serialized_target = serialised
        .get("targets")
        .and_then(serde_json::Value::as_array)
        .and_then(|targets| targets.first())
        .context("serialized manifest should contain one target")?;
    let serialized_command = serialized_target
        .get("Command")
        .and_then(serde_json::Value::as_object)
        .context("serialized target should contain its command recipe")?;
    let serialized_isolation = serialized_command
        .get("command_isolation")
        .and_then(serde_json::Value::as_str);
    match expected {
        CommandIsolation::Shared => ensure!(
            serialized_isolation.is_none(),
            "shared isolation must be omitted from serialisation: {serialised}"
        ),
        CommandIsolation::Isolated => ensure!(
            serialized_isolation == Some("isolated"),
            "isolated mode must be serialised literally: {serialised}"
        ),
    }
    Ok(())
}

#[test]
fn unknown_command_isolation_value_is_rejected() -> Result<()> {
    let yaml = r#"
        netsuke_version: "1.0.0"
        targets:
          - name: lint
            command: [cargo fmt]
            command_isolation: process
    "#;
    let error = parse_manifest(yaml).expect_err("unknown command isolation must be rejected");
    ensure!(
        display_error_chain(error.as_ref()).contains("unknown variant"),
        "unexpected invalid-mode diagnostic: {error}"
    );
    Ok(())
}

#[test]
fn isolated_scalar_command_is_rejected_with_a_localized_message() -> Result<()> {
    let yaml = r#"
        netsuke_version: "1.0.0"
        targets:
          - name: lint
            command: cargo fmt
            command_isolation: isolated
    "#;
    let error = parse_manifest(yaml).expect_err("isolated scalar commands must be rejected");
    let chain = display_error_chain(error.as_ref());
    let expected = localization::message(keys::MANIFEST_COMMAND_ISOLATION_SCALAR).to_string();
    ensure!(
        chain.contains(&expected),
        "unexpected error message: {chain}"
    );
    Ok(())
}

#[rstest::rstest]
#[case::script("script: echo hi")]
#[case::rule("rule: lint")]
#[case::mode_only("command_isolation: shared")]
fn isolation_requires_a_command_list_recipe(#[case] recipe_fields: &str) -> Result<()> {
    let yaml = if recipe_fields.starts_with("command_isolation:") {
        format!("netsuke_version: 1.0.0\ntargets:\n  - name: lint\n    {recipe_fields}\n")
    } else {
        format!(
            "netsuke_version: 1.0.0\ntargets:\n  - name: lint\n    {recipe_fields}\n    command_isolation: shared\n"
        )
    };
    let error = parse_manifest(&yaml).expect_err("isolation requires a command-list recipe");
    let chain = display_error_chain(error.as_ref());
    let expected = localization::message(keys::MANIFEST_COMMAND_ISOLATION_RECIPE).to_string();
    ensure!(
        chain.contains(&expected),
        "unexpected error message: {chain}"
    );
    Ok(())
}

#[test]
fn empty_command_list_is_rejected() -> Result<()> {
    let yaml = r#"
        netsuke_version: "1.0.0"
        rules:
          - name: none
            command: []
        targets:
          - name: hello
            rule: none
    "#;
    let err = parse_manifest(yaml)
        .err()
        .context("an empty command list should fail to parse")?;
    let chain = display_error_chain(err.as_ref());
    let expected = localization::message(keys::MANIFEST_COMMAND_LIST_EMPTY).to_string();
    ensure!(
        chain.contains(&expected),
        "unexpected error message: {chain}"
    );
    Ok(())
}

#[test]
fn direct_ast_deserialization_uses_a_schema_error() -> Result<()> {
    let yaml = r#"
        netsuke_version: "1.0.0"
        rules:
          - name: none
            command: []
        targets:
          - name: hello
            rule: none
    "#;
    let error = serde_saphyr::from_str::<netsuke::ast::NetsukeManifest>(yaml)
        .expect_err("an empty command list should fail AST deserialization");
    ensure!(
        error.to_string().contains("command list must not be empty"),
        "direct AST deserialization should expose the neutral schema error: {error}"
    );
    Ok(())
}
