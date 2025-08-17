//! Tests for Jinja-templated manifest parsing.

use netsuke::{ast::Recipe, manifest};
use rstest::rstest;

fn manifest_yaml(body: &str) -> String {
    format!("netsuke_version: 1.0.0\n{body}")
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
fn expands_foreach_targets() {
    let yaml = manifest_yaml(
        "targets:\n  - foreach: \"['a', 'b']\"\n    name: '{{ item }}'\n    command: 'echo {{ item }}'\n",
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
    assert_eq!(commands, vec!["echo a", "echo b"]);
}

#[rstest]
#[case("[]", "", "no targets should be generated for empty foreach list")]
#[case(
    "['a', 'b']",
    "'false'",
    "no targets should be generated when condition is always false"
)]
fn no_targets_generated_scenarios(
    #[case] foreach_value: &str,
    #[case] when_clause: &str,
    #[case] assertion_message: &str,
) {
    let when_line = if when_clause.is_empty() {
        String::new()
    } else {
        format!("    when: {when_clause}\n")
    };

    let yaml = manifest_yaml(&format!(
        "targets:\n  - foreach: \"{foreach_value}\"\n{when_line}    name: '{{ item }}'\n    command: 'echo {{ item }}'\n",
    ));

    let manifest = manifest::from_str(&yaml).expect("parse");
    assert!(manifest.targets.is_empty(), "{assertion_message}");
}

#[rstest]
fn expands_single_item_foreach_targets() {
    let yaml = manifest_yaml(
        "targets:\n  - foreach: \"['only']\"\n    name: '{{ item }}'\n    command: 'echo {{ item }}'\n",
    );

    let manifest = manifest::from_str(&yaml).expect("parse");
    assert_eq!(
        manifest.targets.len(),
        1,
        "exactly one target should be generated for single-item foreach list"
    );
    let first = manifest.targets.first().expect("target");
    match &first.name {
        netsuke::ast::StringOrList::String(name) => assert_eq!(name, "only"),
        other => panic!("Expected String, got: {other:?}"),
    }
    if let Recipe::Command { command } = &first.recipe {
        assert_eq!(command, "echo only");
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
fn foreach_when_filters_items() {
    let yaml = manifest_yaml(
        "targets:\n  - foreach: \"['a', 'skip', 'b']\"\n    when: item != 'skip'\n    name: '{{ item }}'\n    command: 'echo {{ item }}'\n",
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
}

#[rstest]
fn renders_target_fields_command() {
    let yaml = manifest_yaml(
        "vars:\n  base: base\n  \ntargets:\n  - foreach: \"[1]\"\n    vars:\n      local: '{{ base }}{{ item }}'\n    name: '{{ local }}'\n    sources: ['{{ local }}.src']\n    deps: ['{{ local }}.dep']\n    order_only_deps: ['{{ local }}.ord']\n    command: 'echo {{ local }}'\n",
    );

    let manifest = manifest::from_str(&yaml).expect("parse");
    let target = manifest.targets.first().expect("target");

    match &target.name {
        netsuke::ast::StringOrList::String(n) => assert_eq!(n, "base1"),
        other => panic!("Expected String, got: {other:?}"),
    }

    match &target.sources {
        netsuke::ast::StringOrList::List(list) => assert_eq!(list, &["base1.src".to_string()]),
        other => panic!("Expected List, got: {other:?}"),
    }

    match &target.deps {
        netsuke::ast::StringOrList::List(list) => assert_eq!(list, &["base1.dep".to_string()]),
        other => panic!("Expected List, got: {other:?}"),
    }

    match &target.order_only_deps {
        netsuke::ast::StringOrList::List(list) => {
            assert_eq!(list, &["base1.ord".to_string()]);
        }
        other => panic!("Expected List, got: {other:?}"),
    }

    if let Recipe::Command { command } = &target.recipe {
        assert_eq!(command, "echo base1");
    } else {
        panic!("Expected command recipe, got: {:?}", target.recipe);
    }
}

#[rstest]
fn renders_target_fields_script() {
    let yaml = manifest_yaml(
        "vars:\n  base: base\n  \ntargets:\n  - name: script\n    vars:\n      path: '{{ base }}.sh'\n    script: 'run {{ path }}'\n",
    );

    let manifest = manifest::from_str(&yaml).expect("parse");
    let target = manifest.targets.first().expect("target");
    if let Recipe::Script { script } = &target.recipe {
        assert_eq!(script, "run base.sh");
    } else {
        panic!("Expected script recipe, got: {:?}", target.recipe);
    }
}

#[rstest]
fn renders_target_fields_rule() {
    let yaml = manifest_yaml(
        "vars:\n  base: base\nrules:\n  - name: base-rule\n    command: echo hi\n\ntargets:\n  - name: use-rule\n    rule: '{{ base }}-rule'\n",
    );

    let manifest = manifest::from_str(&yaml).expect("parse");
    let target = manifest.targets.first().expect("target");
    if let Recipe::Rule { rule } = &target.recipe {
        match rule {
            netsuke::ast::StringOrList::String(name) => assert_eq!(name, "base-rule"),
            other => panic!("Expected String, got: {other:?}"),
        }
    } else {
        panic!("Expected rule recipe, got: {:?}", target.recipe);
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
