//! Tests for recipe deserialization: the scalar and list forms of `command`,
//! and the rejection of an empty command list.

use anyhow::{Context, Result, bail, ensure};
use netsuke::ast::{Recipe, StringOrList};
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
        let Recipe::Command { command } = &rule.recipe else {
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
        let Recipe::Command { command } = &rule.recipe else {
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
