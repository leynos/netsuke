//! Unit tests for Netsuke manifest AST deserialisation.

use netsuke::ast::*;
use rstest::rstest;
use semver::Version;

/// Convenience wrapper around `serde_yml::from_str` for manifest tests.
fn parse_manifest(yaml: &str) -> Result<NetsukeManifest, serde_yml::Error> {
    serde_yml::from_str::<NetsukeManifest>(yaml)
}

#[rstest]
fn parse_minimal_manifest() {
    let yaml = r#"netsuke_version: "1.0.0"
targets:
  - name: hello
    recipe:
      kind: command
      command: "echo hi""#;

    let manifest: NetsukeManifest = serde_yml::from_str(yaml).expect("parse");

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
            recipe:
              kind: command
              command: "echo hi"
    "#;
    assert!(serde_yml::from_str::<NetsukeManifest>(yaml).is_err());

    let yaml = r#"
        netsuke_version: "1.0.0"
    "#;
    assert!(serde_yml::from_str::<NetsukeManifest>(yaml).is_err());

    let yaml = r#"
        netsuke_version: "1.0.0"
        targets:
          - recipe:
              kind: command
              command: "echo hi"
    "#;
    assert!(serde_yml::from_str::<NetsukeManifest>(yaml).is_err());
}

#[test]
fn unknown_fields() {
    let yaml = r#"
        netsuke_version: "1.0.0"
        targets:
          - name: hello
            recipe:
              kind: command
              command: "echo hi"
        extra: 42
    "#;
    assert!(serde_yml::from_str::<NetsukeManifest>(yaml).is_err());

    let yaml = r#"
        netsuke_version: "1.0.0"
        targets:
          - name: hello
            recipe:
              kind: command
              command: "echo hi"
            unexpected: true
    "#;
    assert!(serde_yml::from_str::<NetsukeManifest>(yaml).is_err());
}

#[test]
fn empty_lists_and_maps() {
    let yaml = r#"
        netsuke_version: "1.0.0"
        targets: []
    "#;
    let manifest = serde_yml::from_str::<NetsukeManifest>(yaml).expect("parse");
    assert!(manifest.targets.is_empty());

    let yaml = r#"
        netsuke_version: "1.0.0"
        targets:
          - name: hello
            recipe: {}
    "#;
    assert!(serde_yml::from_str::<NetsukeManifest>(yaml).is_err());
}

#[test]
fn string_or_list_variants() {
    let yaml = r#"
        netsuke_version: "1.0.0"
        targets:
          - name: hello
            recipe:
              kind: command
              command: "echo hi"
    "#;
    let manifest = serde_yml::from_str::<NetsukeManifest>(yaml).expect("parse");
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
            recipe:
              kind: command
              command: "echo hi"
    "#;
    let manifest = serde_yml::from_str::<NetsukeManifest>(yaml).expect("parse");
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
            recipe:
              kind: command
              command: "echo hi"
    "#;
    let manifest = serde_yml::from_str::<NetsukeManifest>(yaml).expect("parse");
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
            recipe:
              kind: command
              command: cc
            description: "Compile"
            deps: hello
        targets:
          - name: hello
            recipe:
              kind: rule
              rule: compile
    "#;
    let manifest = serde_yml::from_str::<NetsukeManifest>(yaml).expect("parse");
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
            recipe:
              kind: command
              command: cc
        targets:
          - name: hello
            recipe:
              kind: rule
              rule: compile
    "#;
    let manifest = serde_yml::from_str::<NetsukeManifest>(yaml).expect("parse");
    let rule = manifest.rules.first().expect("rule");
    assert!(rule.description.is_none());
    assert!(matches!(rule.deps, StringOrList::Empty));
}

#[rstest]
#[case::invalid_enum_variant(
    r#"
    netsuke_version: "1.0.0"
    targets:
      - name: hello
        recipe:
          kind: not_a_kind
          command: "echo hi"
"#
)]
#[case::steps_missing_recipe(
    r#"
    netsuke_version: "1.0.0"
    steps:
      - name: setup
    targets:
      - name: done
        recipe:
          kind: command
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
            recipe:
              kind: command
              command: rm -rf build
            phony: true
            always: true
    "#;
    let manifest = serde_yml::from_str::<NetsukeManifest>(yaml).expect("parse");
    let target = manifest.targets.first().expect("target");
    assert!(target.phony);
    assert!(target.always);

    let yaml = r#"
        netsuke_version: "1.0.0"
        targets:
          - name: clean
            recipe:
              kind: command
              command: rm -rf build
    "#;
    let manifest = serde_yml::from_str::<NetsukeManifest>(yaml).expect("parse");
    let target = manifest.targets.first().expect("target");
    assert!(!target.phony);
    assert!(!target.always);
}

#[rstest]
#[case::default_flags(
    r#"
    netsuke_version: "1.0.0"
    steps:
      - name: setup
        recipe:
          kind: command
          command: "echo hi"
    targets:
      - name: done
        recipe:
          kind: command
          command: "true"
"#,
    true,
    false
)]
#[case::explicit_phony_false(
    r#"
    netsuke_version: "1.0.0"
    steps:
      - name: setup
        recipe:
          kind: command
          command: "echo hi"
        phony: false
    targets:
      - name: done
        recipe:
          kind: command
          command: "true"
"#,
    true,
    false
)]
#[case::explicit_always_true(
    r#"
    netsuke_version: "1.0.0"
    steps:
      - name: setup
        recipe:
          kind: command
          command: "echo hi"
        always: true
    targets:
      - name: done
        recipe:
          kind: command
          command: "true"
"#,
    true,
    true
)]
fn steps_behavior(#[case] yaml: &str, #[case] expected_phony: bool, #[case] expected_always: bool) {
    let manifest = parse_manifest(yaml).expect("parse");
    let step = manifest.steps.first().expect("step");
    assert_eq!(step.phony, expected_phony);
    assert_eq!(step.always, expected_always);
}

#[test]
fn multiple_steps_are_marked_phony() {
    let yaml = r#"
        netsuke_version: "1.0.0"
        steps:
          - name: setup
            recipe:
              kind: command
              command: "echo hi"
          - name: build
            recipe:
              kind: command
              command: "make build"
          - name: test
            recipe:
              kind: command
              command: "cargo test"
        targets:
          - name: done
            recipe:
              kind: command
              command: "true"
    "#;
    let manifest = parse_manifest(yaml).expect("parse");
    assert_eq!(manifest.steps.len(), 3);
    for step in &manifest.steps {
        assert!(step.phony);
        assert!(!step.always);
    }
}
