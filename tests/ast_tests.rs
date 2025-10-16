//! Unit tests for Netsuke manifest AST deserialisation.

use anyhow::Result;
use netsuke::{ast::*, manifest};
use rstest::rstest;
use semver::Version;

/// Convenience wrapper around the library manifest parser for tests.
fn parse_manifest(yaml: &str) -> Result<NetsukeManifest> {
    manifest::from_str(yaml)
}

#[rstest]
fn parse_minimal_manifest() {
    let yaml = r#"netsuke_version: "1.0.0"
targets:
  - name: hello
    command: "echo hi""#;

    let manifest = parse_manifest(yaml).expect("parse");

    assert_eq!(
        manifest.netsuke_version,
        Version::parse("1.0.0").expect("ver")
    );
    let first = manifest.targets.first().expect("target");
    let StringOrList::String(name) = &first.name else {
        panic!(
            "Expected target name to be StringOrList::String, got: {:?}",
            first.name
        );
    };
    assert_eq!(name, "hello");

    if let Recipe::Command { command } = &first.recipe {
        assert_eq!(command, "echo hi");
    } else {
        panic!("Expected command recipe, got: {:?}", first.recipe);
    }
}
#[test]
fn missing_required_fields() {
    let yaml = r#"
        targets:
          - name: hello
            command: "echo hi"
    "#;
    assert!(parse_manifest(yaml).is_err());

    let yaml = r#"
        netsuke_version: "1.0.0"
    "#;
    assert!(parse_manifest(yaml).is_err());

    let yaml = r#"
        netsuke_version: "1.0.0"
        targets:
          - command: "echo hi"
    "#;
    assert!(parse_manifest(yaml).is_err());
}

#[test]
fn unknown_fields() {
    let yaml = r#"
        netsuke_version: "1.0.0"
        targets:
          - name: hello
            command: "echo hi"
        extra: 42
    "#;
    assert!(parse_manifest(yaml).is_err());

    let yaml = r#"
        netsuke_version: "1.0.0"
        targets:
          - name: hello
            command: "echo hi"
            unexpected: true
    "#;
    assert!(parse_manifest(yaml).is_err());
}

#[test]
fn empty_lists_and_maps() {
    let yaml = r#"
        netsuke_version: "1.0.0"
        targets: []
    "#;
    let manifest = parse_manifest(yaml).expect("parse");
    assert!(manifest.targets.is_empty());

    let yaml = r#"
        netsuke_version: "1.0.0"
        targets:
          - name: hello
            command: {}
    "#;
    assert!(parse_manifest(yaml).is_err());

    let yaml = r#"
        netsuke_version: "1.0.0"
        targets:
          - name: hello
            script: {}
    "#;
    assert!(parse_manifest(yaml).is_err());

    let yaml = r#"
        netsuke_version: "1.0.0"
        targets:
          - name: hello
            rule: {}
    "#;
    assert!(parse_manifest(yaml).is_err());
}

#[test]
fn string_or_list_variants() {
    let yaml = r#"
        netsuke_version: "1.0.0"
        targets:
          - name: hello
            command: "echo hi"
    "#;
    let manifest = parse_manifest(yaml).expect("parse");
    let first = manifest.targets.first().expect("target");
    if let StringOrList::String(name) = &first.name {
        assert_eq!(name, "hello");
    } else {
        panic!("Expected String variant");
    }

    let yaml = r#"
        netsuke_version: "1.0.0"
        targets:
          - name:
              - hello
              - world
            command: "echo hi"
    "#;
    let manifest = parse_manifest(yaml).expect("parse");
    let first = manifest.targets.first().expect("target");
    if let StringOrList::List(names) = &first.name {
        assert_eq!(names, &vec!["hello".to_string(), "world".to_string()]);
    } else {
        panic!("Expected List variant");
    }

    let yaml = r#"
        netsuke_version: "1.0.0"
        targets:
          - name: []
            command: "echo hi"
    "#;
    let manifest = parse_manifest(yaml).expect("parse");
    let first = manifest.targets.first().expect("target");
    if let StringOrList::List(names) = &first.name {
        assert!(names.is_empty());
    } else {
        panic!("Expected List variant");
    }
}

#[test]
fn optional_fields() {
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
    let manifest = parse_manifest(yaml).expect("parse");
    let rule = manifest.rules.first().expect("rule");
    assert_eq!(rule.description.as_deref(), Some("Compile"));
    match &rule.deps {
        StringOrList::String(dep) => assert_eq!(dep, "hello"),
        other => panic!("deps should be String, got: {other:?}"),
    }

    let yaml = r#"
        netsuke_version: "1.0.0"
        rules:
          - name: compile
            command: cc
        targets:
          - name: hello
            rule: compile
    "#;
    let manifest = parse_manifest(yaml).expect("parse");
    let rule = manifest.rules.first().expect("rule");
    assert!(rule.description.is_none());
    assert!(matches!(rule.deps, StringOrList::Empty));
}

#[rstest]
fn parses_macro_definitions() {
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

    let manifest = parse_manifest(yaml).expect("parse");
    assert_eq!(manifest.macros.len(), 1);
    let macro_def = manifest.macros.first().expect("macro");
    assert_eq!(macro_def.signature, "greet(name)");
    assert!(macro_def.body.contains("Hello {{ name }}"));

    let serialised = serde_yml::to_string(&manifest.macros).expect("serialise macros");
    assert!(serialised.contains("greet(name)"));
    assert!(serialised.contains("Hello {{ name }}"));
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
fn phony_and_always_flags() {
    let yaml = r#"
        netsuke_version: "1.0.0"
        targets:
          - name: clean
            command: rm -rf build
            phony: true
            always: true
    "#;
    let manifest = parse_manifest(yaml).expect("parse");
    let target = manifest.targets.first().expect("target");
    assert!(target.phony);
    assert!(target.always);

    let yaml = r#"
        netsuke_version: "1.0.0"
        targets:
          - name: clean
            command: rm -rf build
    "#;
    let manifest = parse_manifest(yaml).expect("parse");
    let target = manifest.targets.first().expect("target");
    assert!(!target.phony);
    assert!(!target.always);
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
) {
    let manifest = parse_manifest(yaml).expect("parse");
    let action = manifest.actions.first().expect("action");
    assert_eq!(action.phony, expected_phony);
    assert_eq!(action.always, expected_always);
}

#[test]
fn multiple_actions_are_marked_phony() {
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
    let manifest = parse_manifest(yaml).expect("parse");
    assert_eq!(manifest.actions.len(), 3);
    for action in &manifest.actions {
        assert!(action.phony);
        assert!(!action.always);
    }
}

#[test]
fn load_manifest_from_file() {
    let manifest = manifest::from_path("tests/data/minimal.yml").expect("load");
    assert_eq!(
        manifest.netsuke_version,
        Version::parse("1.0.0").expect("ver")
    );
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
fn parse_example_manifests(#[case] file: &str, #[case] first_target: &str) {
    let path = format!("tests/data/{file}");
    let manifest = manifest::from_path(&path).expect("load");
    let first = manifest.targets.first().expect("targets");
    match &first.name {
        StringOrList::String(name) => assert_eq!(name, first_target),
        other => panic!("Expected String variant, got: {other:?}"),
    }
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
