//! Tests for `macros:` definitions: parsing, serialization round-trips, and
//! rejection of ill-typed entries.

use anyhow::{Context, Result, ensure};
use netsuke::ast::MacroDefinition;
use rstest::rstest;

use super::support::parse_manifest;

#[rstest]
fn parses_macro_definitions() -> Result<()> {
    let yaml = r#"
        netsuke_version: "1.0.0"
        macros:
          - signature: "greet(name)"
            body: |-
              Hello {{ name }}
        targets:
          - name: hello
            command: "{{ greet('world') }}"
    "#;

    let manifest = parse_manifest(yaml)?;
    ensure!(
        manifest.macros.len() == 1,
        "expected single macro definition"
    );
    let macro_def = manifest
        .macros
        .first()
        .context("expected at least one macro definition")?;
    ensure!(
        macro_def.signature == "greet(name)",
        "unexpected macro signature: {}",
        macro_def.signature
    );
    ensure!(
        macro_def.body.contains("Hello {{ name }}"),
        "macro body missing greeting: {}",
        macro_def.body
    );

    let serialized = serde_saphyr::to_string(&manifest.macros)?;
    ensure!(
        serialized.contains("greet(name)"),
        "serialized macros missing signature: {serialized}"
    );
    ensure!(
        serialized.contains("Hello {{ name }}"),
        "serialized macros missing body: {serialized}"
    );
    Ok(())
}

#[test]
fn macro_serialization_with_special_characters_round_trips() -> Result<()> {
    let special_signature = "greet_special(name, emoji='😀', note=\"hi\")";
    let special_body = "Hello \"{{ name }}\"\nLine two with unicode 😀";

    let macro_def = MacroDefinition {
        signature: special_signature.to_owned(),
        body: special_body.to_owned(),
    };

    let serialized = serde_saphyr::to_string(&vec![macro_def.clone()])?;
    ensure!(
        serialized.contains("greet_special"),
        "serialized macros missing signature: {serialized}"
    );
    ensure!(
        serialized.contains("unicode 😀"),
        "serialized macros missing unicode glyph: {serialized}"
    );

    let deserialized: Vec<MacroDefinition> = serde_saphyr::from_str(&serialized)?;
    ensure!(deserialized.len() == 1, "expected single macro entry");
    let recovered = deserialized
        .first()
        .context("expected macro entry after round trip")?;
    ensure!(
        recovered.signature == macro_def.signature,
        "signature mismatch: got {}, expected {}",
        recovered.signature,
        macro_def.signature
    );
    ensure!(
        recovered.body == macro_def.body,
        "body mismatch: got {}, expected {}",
        recovered.body,
        macro_def.body
    );
    Ok(())
}

#[test]
fn macro_definition_rejects_invalid_types() {
    let yaml = r#"
        netsuke_version: "1.0.0"
        macros:
          - signature: 42
            body: []
        targets:
          - name: hello
            command: noop
    "#;

    assert!(
        parse_manifest(yaml).is_err(),
        "non-string macro signature and body should be rejected"
    );
}
