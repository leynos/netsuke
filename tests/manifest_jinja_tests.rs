//! Tests for the YAML-first manifest pipeline: parse YAML, expand foreach/when,
//! then render Jinja only in string values.

use netsuke::{ast::Recipe, manifest};
use rstest::rstest;

fn manifest_yaml(body: &str) -> String {
    format!("netsuke_version: 1.0.0\n{body}")
}

fn assert_string_or_list_eq(actual: &netsuke::ast::StringOrList, expected: &str, field: &str) {
    match actual {
        netsuke::ast::StringOrList::String(s) => assert_eq!(s, expected),
        netsuke::ast::StringOrList::List(list) if list.len() == 1 => {
            assert_eq!(list.first().expect("list"), expected);
        }
        other => panic!("Expected String or single-item List for {field}, got: {other:?}"),
    }
}

fn assert_string_or_list_eq_list(
    actual: &netsuke::ast::StringOrList,
    expected: &[String],
    field: &str,
) {
    match actual {
        netsuke::ast::StringOrList::List(list) => assert_eq!(list, expected),
        other => panic!("Expected List for {field}, got: {other:?}"),
    }
}

fn extract_target_field<F>(manifest: &netsuke::ast::NetsukeManifest, extract: F) -> Vec<String>
where
    F: Fn(&netsuke::ast::Target) -> String,
{
    manifest.targets.iter().map(extract).collect()
}

fn extract_target_names(manifest: &netsuke::ast::NetsukeManifest) -> Vec<String> {
    extract_target_field(manifest, |t| match &t.name {
        netsuke::ast::StringOrList::String(s) => s.clone(),
        other => panic!("Expected String, got: {other:?}"),
    })
}

fn extract_target_commands(manifest: &netsuke::ast::NetsukeManifest) -> Vec<String> {
    extract_target_field(manifest, |t| match &t.recipe {
        Recipe::Command { command } => command.clone(),
        other => panic!("Expected command recipe, got: {other:?}"),
    })
}

#[rstest]
fn renders_global_vars() {
    let yaml = manifest_yaml(
        "vars:\n  who: world\ntargets:\n  - name: hello\n    command: echo {{ who }}\n",
    );

    let manifest = manifest::from_str(&yaml).expect("parse");
    let first = manifest.targets.first().expect("target");
    if let Recipe::Command { command } = &first.recipe {
        assert_eq!(command, "echo world");
    } else {
        panic!("Expected command recipe, got: {:?}", first.recipe);
    }
}

#[rstest]
fn undefined_variable_errors() {
    let yaml = manifest_yaml("targets:\n  - name: hello\n    command: echo {{ missing }}\n");

    assert!(manifest::from_str(&yaml).is_err());
}

#[rstest]
fn syntax_error_errors() {
    let yaml = manifest_yaml("targets:\n  - name: hello\n    command: echo {{ who\n");

    assert!(manifest::from_str(&yaml).is_err());
}

#[rstest]
#[case(true, "echo on")]
#[case(false, "echo off")]
fn renders_if_blocks(#[case] flag: bool, #[case] expected: &str) {
    let cmd = "{% if flag %}echo on{% else %}echo off{% endif %}";
    let yaml = manifest_yaml(&format!(
        concat!(
            "vars:\n",
            "  flag: {flag}\n",
            "targets:\n",
            "  - name: test\n",
            "    command: \"{cmd}\"\n",
        ),
        flag = flag,
        cmd = cmd,
    ));

    let manifest = manifest::from_str(&yaml).expect("parse");
    let first = manifest.targets.first().expect("target");
    if let Recipe::Command { command } = &first.recipe {
        assert_eq!(command, expected);
    } else {
        panic!("Expected command recipe, got: {:?}", first.recipe);
    }
}

#[rstest]
#[case(
    concat!(
        "targets:\n",
        "  - foreach:\n",
        "      - a\n",
        "      - b\n",
        "    name: '{{ item }}'\n",
        "    command: \"echo '{{ item }}'\"\n",
    ),
    vec!["a", "b"],
    vec!["echo 'a'", "echo 'b'"],
)]
#[case(
    concat!(
        "targets:\n",
        "  - foreach: \"['x', 'y']\"\n",
        "    name: '{{ index }}:{{ item }}'\n",
        "    command: 'echo {{ index }} {{ item }}'\n",
    ),
    vec!["0:x", "1:y"],
    vec!["echo 0 x", "echo 1 y"],
)]
fn expands_foreach_with_item_and_index(
    #[case] yaml_body: &str,
    #[case] expected_names: Vec<&str>,
    #[case] expected_commands: Vec<&str>,
) {
    let yaml = manifest_yaml(yaml_body);

    let manifest = manifest::from_str(&yaml).expect("parse");

    let names = extract_target_names(&manifest);
    assert_eq!(
        names,
        expected_names
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
    );

    let commands = extract_target_commands(&manifest);
    assert_eq!(
        commands,
        expected_commands
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
    );
}

#[rstest]
#[case(
    "[]",
    "",
    "no targets should be generated for empty foreach list",
    true
)]
#[case(
    "['a', 'b']",
    "false",
    "no targets should be generated when condition is always false",
    true
)]
#[case(
    "[]",
    "",
    "no targets should be generated for empty foreach list (typed)",
    false
)]
fn no_targets_generated_scenarios(
    #[case] foreach_value: &str,
    #[case] when_clause: &str,
    #[case] assertion_message: &str,
    #[case] quoted_foreach: bool,
) {
    let when_line = if when_clause.is_empty() {
        String::new()
    } else {
        format!("    when: \"{when_clause}\"\n")
    };

    let foreach_lit = if quoted_foreach {
        format!("\"{foreach_value}\"")
    } else {
        foreach_value.to_string()
    };

    let yaml = manifest_yaml(&format!(
        "targets:\n  - foreach: {foreach_lit}\n{when_line}    name: '{{ item }}'\n    command: 'echo {{ item }}'\n",
    ));

    let manifest = manifest::from_str(&yaml).expect("parse");
    assert!(manifest.targets.is_empty(), "{assertion_message}");
}

#[rstest]
fn expands_single_item_foreach_targets() {
    let yaml = manifest_yaml(
        "targets:\n  - foreach:\n      - only\n    name: '{{ item }}'\n    command: \"echo '{{ item }}'\"\n",
    );

    let manifest = manifest::from_str(&yaml).expect("parse");
    assert_eq!(
        manifest.targets.len(),
        1,
        "exactly one target should be generated for single-item foreach list"
    );
    let first = manifest.targets.first().expect("target");
    assert_string_or_list_eq(&first.name, "only", "name");
    if let Recipe::Command { command } = &first.recipe {
        assert_eq!(command, "echo 'only'");
    } else {
        panic!("Expected command recipe, got: {:?}", first.recipe);
    }
}

#[rstest]
#[case("1", true)] // foreach: "1"
#[case("1", false)] // foreach: 1
fn foreach_non_iterable_errors(#[case] val: &str, #[case] quoted: bool) {
    let foreach = if quoted {
        format!("\"{val}\"")
    } else {
        val.to_string()
    };
    let yaml = manifest_yaml(&format!(
        "targets:\n  - foreach: {foreach}\n    name: 'a'\n    command: 'echo a'\n",
    ));

    assert!(manifest::from_str(&yaml).is_err());
}

#[rstest]
fn foreach_vars_must_be_mapping() {
    let yaml = manifest_yaml(
        "targets:\n  - foreach: ['a']\n    vars: 1\n    name: 'x'\n    command: 'echo x'\n",
    );

    let err = manifest::from_str(&yaml).expect_err("parse should fail");
    assert!(
        err.to_string().contains("target.vars must be a mapping"),
        "unexpected error: {err}"
    );
}

#[rstest]
fn foreach_when_filters_items() {
    let yaml = manifest_yaml(
        "targets:\n  - foreach:\n      - a\n      - skip\n      - b\n    when: item != 'skip'\n    name: '{{ item }}'\n    command: \"echo '{{ item }}'\"\n",
    );

    let manifest = manifest::from_str(&yaml).expect("parse");
    assert_eq!(manifest.targets.len(), 2);
    let names: Vec<_> = manifest
        .targets
        .iter()
        .map(|t| match &t.name {
            netsuke::ast::StringOrList::String(s) => s.clone(),
            other => panic!("Expected String, got: {other:?}"),
        })
        .collect();
    assert_eq!(names, vec!["a", "b"]);

    let commands: Vec<_> = manifest
        .targets
        .iter()
        .map(|t| match &t.recipe {
            Recipe::Command { command } => command.clone(),
            other => panic!("Expected command recipe, got: {other:?}"),
        })
        .collect();
    assert_eq!(commands, vec!["echo 'a'", "echo 'b'"]);
}

#[rstest]
fn renders_target_fields_command() {
    let yaml = manifest_yaml(
        "vars:\n  base: base\n  \ntargets:\n  - foreach:\n      - 1\n    vars:\n      local: '{{ base }}{{ item }}'\n    name: '{{ local }}'\n    sources: ['{{ local }}.src']\n    deps: ['{{ local }}.dep']\n    order_only_deps: ['{{ local }}.ord']\n    command: \"echo '{{ local }}'\"\n",
    );

    let manifest = manifest::from_str(&yaml).expect("parse");
    let target = manifest.targets.first().expect("target");
    assert_string_or_list_eq(&target.name, "base1", "name");
    assert_string_or_list_eq_list(&target.sources, &["base1.src".to_string()], "sources");
    assert_string_or_list_eq_list(&target.deps, &["base1.dep".to_string()], "deps");
    assert_string_or_list_eq_list(
        &target.order_only_deps,
        &["base1.ord".to_string()],
        "order_only_deps",
    );
    if let Recipe::Command { command } = &target.recipe {
        assert_eq!(command, "echo 'base1'");
    } else {
        panic!("Expected command recipe, got: {:?}", target.recipe);
    }
}

#[rstest]
#[case(
    "script",
    "run base.sh",
    "vars:\n  base: base\n  \ntargets:\n  - name: script\n    vars:\n      path: '{{ base }}.sh'\n    script: 'run {{ path }}'\n"
)]
#[case(
    "rule",
    "base-rule",
    "vars:\n  base: base\nrules:\n  - name: base-rule\n    command: echo hi\n\ntargets:\n  - name: use-rule\n    rule: '{{ base }}-rule'\n"
)]
fn renders_target_fields_recipe_types(
    #[case] recipe_type: &str,
    #[case] expected_value: &str,
    #[case] yaml_body: &str,
) {
    let yaml = manifest_yaml(yaml_body);

    let manifest = manifest::from_str(&yaml).expect("parse");
    let target = manifest.targets.first().expect("target");
    match (recipe_type, &target.recipe) {
        ("script", Recipe::Script { script }) => assert_eq!(script, expected_value),
        ("rule", Recipe::Rule { rule }) => {
            assert_string_or_list_eq(rule, expected_value, "rule");
        }
        ("script", recipe) => panic!("Expected script recipe, got: {recipe:?}"),
        ("rule", recipe) => panic!("Expected rule recipe, got: {recipe:?}"),
        (other, _) => panic!("Unexpected recipe type: {other}"),
    }
}

#[rstest]
fn render_target_missing_var_errors() {
    let yaml = manifest_yaml(
        "targets:\n  - name: test\n    sources: ['{{ missing }}']\n    command: echo hi\n",
    );

    assert!(manifest::from_str(&yaml).is_err());
}

#[rstest]
fn undefined_in_if_errors() {
    let yaml = manifest_yaml(
        "targets:\n  - name: test\n    command: \"{% if missing %}echo hi{% endif %}\"\n",
    );

    assert!(manifest::from_str(&yaml).is_err());
}
