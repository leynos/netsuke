//! Unit tests for Netsuke manifest AST deserialisation.

use anyhow::{Context, Result, bail, ensure};
use netsuke::localization::keys;
use netsuke::{ast::*, localization, manifest};
use rstest::rstest;
use semver::Version;

/// Convenience wrapper around the library manifest parser for tests.
fn parse_manifest(yaml: &str) -> Result<NetsukeManifest> {
    manifest::from_str(yaml)
}

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
        ensure!(command == "echo hi", "unexpected command: {command}");
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
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    let expected = localization::message(keys::MANIFEST_VARS_NOT_OBJECT).to_string();
    ensure!(
        chain.contains(&expected),
        "unexpected error message: {chain}"
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
fn string_or_list_variants() -> Result<()> {
    {
        let yaml = r#"
            netsuke_version: "1.0.0"
            targets:
              - name: hello
                command: "echo hi"
        "#;
        let manifest = parse_manifest(yaml)?;
        let first = manifest
            .targets
            .first()
            .context("manifest should contain at least one target")?;
        match &first.name {
            StringOrList::String(name) => {
                ensure!(name == "hello", "unexpected name: {name}");
            }
            other => bail!("Expected String variant, got: {other:?}"),
        }
    }

    {
        let yaml = r#"
            netsuke_version: "1.0.0"
            targets:
              - name:
                  - hello
                  - world
                command: "echo hi"
        "#;
        let manifest = parse_manifest(yaml)?;
        let first = manifest
            .targets
            .first()
            .context("manifest should contain at least one target")?;
        match &first.name {
            StringOrList::List(names) => {
                let expected = vec!["hello".to_owned(), "world".to_owned()];
                ensure!(
                    names == &expected,
                    "unexpected names: got {:?}, expected {:?}",
                    names,
                    expected
                );
            }
            other => bail!("Expected List variant, got: {other:?}"),
        }
    }

    {
        let yaml = r#"
            netsuke_version: "1.0.0"
            targets:
              - name: []
                command: "echo hi"
        "#;
        let manifest = parse_manifest(yaml)?;
        let first = manifest
            .targets
            .first()
            .context("manifest should contain at least one target")?;
        match &first.name {
            StringOrList::List(names) => {
                ensure!(names.is_empty(), "expected empty list, got {names:?}");
            }
            other => bail!("Expected List variant, got: {other:?}"),
        }
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
                deps: hello
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
        match &rule.deps {
            StringOrList::String(dep) => {
                ensure!(dep == "hello", "unexpected dep: {dep}");
            }
            other => bail!("deps should be String, got: {other:?}"),
        }
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
        ensure!(
            matches!(rule.deps, StringOrList::Empty),
            "deps should be empty"
        );
    }
    Ok(())
}

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

    let serialised = serde_saphyr::to_string(&manifest.macros)?;
    ensure!(
        serialised.contains("greet(name)"),
        "serialised macros missing signature: {serialised}"
    );
    ensure!(
        serialised.contains("Hello {{ name }}"),
        "serialised macros missing body: {serialised}"
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

    let serialised = serde_saphyr::to_string(&vec![macro_def.clone()])?;
    ensure!(
        serialised.contains("greet_special"),
        "serialised macros missing signature: {serialised}"
    );
    ensure!(
        serialised.contains("unicode 😀"),
        "serialised macros missing unicode glyph: {serialised}"
    );

    let deserialised: Vec<MacroDefinition> = serde_saphyr::from_str(&serialised)?;
    ensure!(deserialised.len() == 1, "expected single macro entry");
    let recovered = deserialised
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

    assert!(parse_manifest(yaml).is_err());
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
    assert!(parse_manifest(yaml).is_err());
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

#[rstest]
#[case::default_flags(
    r#"
    netsuke_version: "1.0.0"
    actions:
      - name: setup
        command: "echo hi"
    targets:
      - name: done
        command: "true"
"#,
    true,
    false
)]
#[case::explicit_phony_false(
    r#"
    netsuke_version: "1.0.0"
    actions:
      - name: setup
        command: "echo hi"
        phony: false
    targets:
      - name: done
        command: "true"
"#,
    true,
    false
)]
#[case::explicit_always_true(
    r#"
    netsuke_version: "1.0.0"
    actions:
      - name: setup
        command: "echo hi"
        always: true
    targets:
      - name: done
        command: "true"
"#,
    true,
    true
)]
fn actions_behaviour(
    #[case] yaml: &str,
    #[case] expected_phony: bool,
    #[case] expected_always: bool,
) -> Result<()> {
    let manifest = parse_manifest(yaml)?;
    let action = manifest.actions.first().context("expected action entry")?;
    ensure!(
        action.phony == expected_phony,
        "unexpected phony flag: got {}, expected {}",
        action.phony,
        expected_phony
    );
    ensure!(
        action.always == expected_always,
        "unexpected always flag: got {}, expected {}",
        action.always,
        expected_always
    );
    Ok(())
}

#[test]
fn multiple_actions_are_marked_phony() -> Result<()> {
    let yaml = r#"
        netsuke_version: "1.0.0"
        actions:
          - name: setup
            command: "echo hi"
          - name: build
            command: "make build"
          - name: test
            command: "cargo test"
        targets:
          - name: done
            command: "true"
    "#;
    let manifest = parse_manifest(yaml)?;
    ensure!(
        manifest.actions.len() == 3,
        "expected three actions, got {}",
        manifest.actions.len()
    );
    for action in &manifest.actions {
        ensure!(action.phony, "all actions should be phony");
        ensure!(!action.always, "actions should not always run");
    }
    Ok(())
}

#[test]
fn load_manifest_from_file() -> Result<()> {
    let manifest = manifest::from_path("tests/data/minimal.yml")?;
    let expected_version = Version::parse("1.0.0")?;
    ensure!(
        manifest.netsuke_version == expected_version,
        "unexpected manifest version: got {}, expected {}",
        manifest.netsuke_version,
        expected_version
    );
    Ok(())
}

#[test]
fn load_manifest_missing_file() {
    let result = manifest::from_path("tests/data/missing.yml");
    assert!(result.is_err());
}

#[rstest]
#[case("minimal.yml", "hello")]
#[case("phony.yml", "clean")]
#[case("rules.yml", "hello.o")]
fn parse_example_manifests(#[case] file: &str, #[case] first_target: &str) -> Result<()> {
    let path = format!("tests/data/{file}");
    let manifest = manifest::from_path(&path)?;
    let first = manifest
        .targets
        .first()
        .context("expected target entry in manifest")?;
    match &first.name {
        StringOrList::String(name) => {
            ensure!(name == first_target, "unexpected name: {name}");
        }
        other => bail!("Expected String variant, got: {other:?}"),
    }
    Ok(())
}

#[rstest]
#[case("unknown_field.yml")]
#[case("invalid_version.yml")]
#[case("missing_recipe.yml")]
#[case("action_invalid.yml")]
fn invalid_manifests_fail(#[case] file: &str) {
    let path = format!("tests/data/{file}");
    assert!(manifest::from_path(&path).is_err());
}
