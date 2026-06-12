//! Conditional expansion cases for manifest entries.

use super::*;
use anyhow::{Context, Result};
use minijinja::Environment;
use rstest::rstest;

#[rstest]
#[case::targets("targets")]
#[case::actions("actions")]
fn expand_static_when_false_removes_entry_before_typed_ast(#[case] section: &str) -> Result<()> {
    let env = Environment::new();
    let yaml = format!(
        "{section}:
  - name: skipped
    command: echo skipped
    when: 'false'
  - name: kept
    command: echo kept"
    );
    let mut doc: ManifestValue = serde_saphyr::from_str(&yaml)?;
    expand_foreach(&mut doc, &env)?;
    let entries = section_entries(&doc, section)?;
    anyhow::ensure!(entries.len() == 1, "expected one kept {section} entry");
    let map = entries
        .first()
        .and_then(ManifestValue::as_object)
        .with_context(|| format!("{section} entry map"))?;
    let name = map
        .get("name")
        .and_then(ManifestValue::as_str)
        .with_context(|| format!("{section} entry name"))?;
    anyhow::ensure!(name == "kept", "unexpected kept {section} name: {name}");
    anyhow::ensure!(
        !map.contains_key("when"),
        "when should be removed before typed AST deserialization"
    );
    Ok(())
}

#[rstest]
#[case::targets("targets")]
#[case::actions("actions")]
fn expand_foreach_when_injects_iteration_vars_only_for_kept_entries(
    #[case] section: &str,
) -> Result<()> {
    let env = Environment::new();
    let yaml = format!(
        "{section}:
  - foreach:
      - skip
      - keep
      - also-keep
    when: item != 'skip'
    name: '{{{{ item }}}}'
    command: echo {{{{ item }}}}"
    );
    let mut doc: ManifestValue = serde_saphyr::from_str(&yaml)?;
    expand_foreach(&mut doc, &env)?;
    let entries = section_entries(&doc, section)?;
    anyhow::ensure!(entries.len() == 2, "expected two kept {section} entries");
    anyhow::ensure!(
        indexes(entries, section)? == vec![1, 2],
        "indexes should preserve original iteration positions"
    );
    let names: Result<Vec<_>> = entries
        .iter()
        .map(|entry| {
            entry
                .as_object()
                .and_then(|map| map.get("name"))
                .and_then(ManifestValue::as_str)
                .map(str::to_owned)
                .with_context(|| format!("{section} name"))
        })
        .collect();
    let expected_names = vec!["{{ item }}".to_owned(), "{{ item }}".to_owned()];
    anyhow::ensure!(
        names? == expected_names,
        "final string rendering should happen after expansion"
    );
    for entry in entries {
        let map = entry
            .as_object()
            .with_context(|| format!("{section} entry map"))?;
        anyhow::ensure!(
            !map.contains_key("foreach"),
            "foreach should be removed before typed AST deserialization"
        );
        anyhow::ensure!(
            !map.contains_key("when"),
            "when should be removed before typed AST deserialization"
        );
    }
    Ok(())
}

#[test]
fn expand_static_when_can_read_entry_vars() -> Result<()> {
    let env = Environment::new();
    let yaml = "targets:
  - name: kept
    vars:
      enabled: true
    when: enabled
  - name: skipped
    vars:
      enabled: false
    when: enabled";
    let mut doc: ManifestValue = serde_saphyr::from_str(yaml)?;
    expand_foreach(&mut doc, &env)?;
    let targets = targets(&doc)?;
    anyhow::ensure!(targets.len() == 1, "expected one kept target");
    let name = targets
        .first()
        .and_then(ManifestValue::as_object)
        .and_then(|map| map.get("name"))
        .and_then(ManifestValue::as_str)
        .context("kept target name")?;
    anyhow::ensure!(name == "kept", "unexpected kept target: {name}");
    Ok(())
}

#[test]
fn expand_foreach_when_item_overrides_entry_vars() -> Result<()> {
    let env = Environment::new();
    let yaml = "targets:
  - name: literal
    foreach:
      - keep
      - skip
    vars:
      item: skip
    when: item == 'keep'";
    let mut doc: ManifestValue = serde_saphyr::from_str(yaml)?;
    expand_foreach(&mut doc, &env)?;
    let targets = targets(&doc)?;
    anyhow::ensure!(targets.len() == 1, "expected one kept target");
    anyhow::ensure!(indexes(targets, "target")? == vec![0], "wrong index");
    Ok(())
}

#[test]
fn expand_foreach_expands_sequence_values() -> Result<()> {
    let env = Environment::new();
    let mut doc: ManifestValue = serde_saphyr::from_str(
        "targets:
  - name: literal
    foreach:
      - 1
      - 2
    vars:
      static: keep",
    )?;
    expand_foreach(&mut doc, &env)?;
    let targets = targets(&doc)?;
    anyhow::ensure!(targets.len() == 2, "expected two targets");
    for (idx, target) in targets.iter().enumerate() {
        let map = target.as_object().context("target map")?;
        anyhow::ensure!(
            !map.contains_key("foreach"),
            "foreach should be removed after target expansion"
        );
        let vars = map
            .get("vars")
            .and_then(|v| v.as_object())
            .context("vars map")?;
        let index_val = vars.get("index").context("index value")?;
        let item_val = vars.get("item").context("item value")?;
        let ManifestValue::Number(index_num) = index_val else {
            anyhow::bail!("index should be numeric: {index_val:?}");
        };
        let index = index_num
            .as_u64()
            .context("numeric index conversion failed")?;
        anyhow::ensure!(index == idx as u64, "unexpected index value: {index}");
        let ManifestValue::Number(item_num) = item_val else {
            anyhow::bail!("item should be numeric: {item_val:?}");
        };
        let item = item_num
            .as_u64()
            .context("numeric item conversion failed")?;
        anyhow::ensure!(item == (idx + 1) as u64, "unexpected item value: {item}");
    }
    Ok(())
}

#[test]
fn expand_foreach_applies_when_expression() -> Result<()> {
    let env = Environment::new();
    let mut doc: ManifestValue = serde_saphyr::from_str(
        "targets:
  - name: literal
    foreach: '[1, 2, 3]'
    when: 'item > 1'",
    )?;
    expand_foreach(&mut doc, &env)?;
    let targets = targets(&doc)?;
    anyhow::ensure!(targets.len() == 2, "expected filtered targets");
    let indexes = indexes(targets, "target")?;
    anyhow::ensure!(
        indexes == vec![1, 2],
        "unexpected filtered indexes: {:?}",
        indexes
    );
    for target in targets {
        let map = target.as_object().context("target map")?;
        anyhow::ensure!(
            !map.contains_key("foreach"),
            "foreach should be removed from filtered targets"
        );
    }
    Ok(())
}

#[test]
fn expand_foreach_expands_actions_sequence_values() -> Result<()> {
    let env = Environment::new();
    let yaml = "actions:
  - name: literal
    foreach:
      - alpha
      - beta
    command: echo {{ item }}
    vars:
      static: keep";
    let mut doc: ManifestValue = serde_saphyr::from_str(yaml)?;
    expand_foreach(&mut doc, &env)?;
    let actions = actions(&doc)?;
    anyhow::ensure!(actions.len() == 2, "expected two actions");
    anyhow::ensure!(indexes(actions, "action")? == vec![0, 1], "wrong indexes");
    for action in actions {
        let map = action.as_object().context("action map")?;
        anyhow::ensure!(
            !map.contains_key("foreach"),
            "foreach should be removed after action expansion"
        );
    }
    Ok(())
}

#[test]
fn expand_foreach_applies_action_when_expression() -> Result<()> {
    let env = Environment::new();
    let yaml = "actions:
  - name: literal
    command: echo {{ item }}
    foreach: '[1, 2, 3]'
    when: 'item > 1'";
    let mut doc: ManifestValue = serde_saphyr::from_str(yaml)?;
    expand_foreach(&mut doc, &env)?;
    let actions = actions(&doc)?;
    anyhow::ensure!(actions.len() == 2, "expected filtered actions");
    anyhow::ensure!(indexes(actions, "action")? == vec![1, 2], "wrong indexes");
    for action in actions {
        let map = action.as_object().context("action map")?;
        anyhow::ensure!(
            !map.contains_key("foreach"),
            "foreach should be removed from filtered actions"
        );
    }
    Ok(())
}

#[test]
fn expand_foreach_empty_foreach_produces_no_entries() -> Result<()> {
    let env = Environment::new();
    let mut doc: ManifestValue = serde_saphyr::from_str(
        "targets:
  - name: literal
    foreach: []
    command: echo hi",
    )?;
    expand_foreach(&mut doc, &env)?;
    let targets = targets(&doc)?;
    anyhow::ensure!(
        targets.is_empty(),
        "empty foreach should expand to no targets: {targets:?}"
    );
    Ok(())
}

#[test]
fn expand_foreach_non_object_entry_is_passed_through() -> Result<()> {
    let env = Environment::new();
    let mut doc: ManifestValue = serde_saphyr::from_str(
        "targets:
  - just-a-string
  - name: real
    command: echo hi",
    )?;
    expand_foreach(&mut doc, &env)?;
    let targets = targets(&doc)?;
    anyhow::ensure!(targets.len() == 2, "expected both entries to survive");
    anyhow::ensure!(
        targets.first().and_then(ManifestValue::as_str) == Some("just-a-string"),
        "bare string entry should pass through unexpanded: {:?}",
        targets.first()
    );
    Ok(())
}

#[test]
fn expand_foreach_iteration_vars_do_not_get_overwritten_by_entry_vars() -> Result<()> {
    let env = Environment::new();
    let mut doc: ManifestValue = serde_saphyr::from_str(
        "targets:
  - name: literal
    foreach:
      - from-iteration
    vars:
      item: from-entry
      other: untouched",
    )?;
    expand_foreach(&mut doc, &env)?;
    let targets = targets(&doc)?;
    anyhow::ensure!(targets.len() == 1, "expected one expanded target");
    let vars = targets
        .first()
        .and_then(ManifestValue::as_object)
        .and_then(|map| map.get("vars"))
        .and_then(ManifestValue::as_object)
        .context("vars map")?;
    // `inject_iteration_vars` inserts `item`/`index` after cloning the entry,
    // so the iteration-injected value takes precedence over the entry's own
    // `item` var while unrelated vars survive.
    anyhow::ensure!(
        vars.get("item").and_then(ManifestValue::as_str) == Some("from-iteration"),
        "iteration item should override the entry's own item var: {vars:?}"
    );
    anyhow::ensure!(
        vars.get("other").and_then(ManifestValue::as_str) == Some("untouched"),
        "unrelated entry vars should survive expansion: {vars:?}"
    );
    Ok(())
}

#[test]
fn expand_foreach_jinja_filter_in_name() -> Result<()> {
    // Name rendering happens in the full pipeline (render_manifest), so this
    // test drives manifest::from_str rather than expand_foreach directly.
    let manifest = crate::manifest::from_str(
        "netsuke_version: \"1.0.0\"
targets:
  - name: '{{ item | upper }}'
    foreach:
      - alpha
      - beta
    command: echo hi",
    )?;
    let names: Vec<&str> = manifest
        .targets
        .iter()
        .map(|t| match &t.name {
            crate::ast::StringOrList::String(s) => Ok(s.as_str()),
            other => Err(anyhow::anyhow!("expected string name, got {other:?}")),
        })
        .collect::<Result<_>>()?;
    anyhow::ensure!(
        names == ["ALPHA", "BETA"],
        "expected uppercased names from Jinja filter: {names:?}"
    );
    Ok(())
}

#[test]
fn expand_static_action_when_false_drops_action() -> Result<()> {
    let env = Environment::new();
    let yaml = "actions:
  - name: skipped
    command: echo skipped
    when: 'false'
  - name: kept
    command: echo kept";
    let mut doc: ManifestValue = serde_saphyr::from_str(yaml)?;
    expand_foreach(&mut doc, &env)?;
    let actions = actions(&doc)?;
    anyhow::ensure!(actions.len() == 1, "expected one action");
    let map = actions
        .first()
        .and_then(ManifestValue::as_object)
        .context("action map")?;
    let name = map
        .get("name")
        .and_then(ManifestValue::as_str)
        .context("action name")?;
    anyhow::ensure!(name == "kept", "unexpected action name: {name}");
    anyhow::ensure!(
        !map.contains_key("when"),
        "when should be removed after action expansion"
    );
    Ok(())
}

#[rstest]
#[case::present("preferred-tool", "preferred")]
#[case::absent("missing-tool", "fallback")]
fn expand_static_action_when_supports_complementary_command_available_branches(
    #[case] command_name: &str,
    #[case] expected_name: &str,
) -> Result<()> {
    let mut env = Environment::new();
    env.add_function("command_available", |name: String| {
        Ok::<bool, minijinja::Error>(name == "preferred-tool")
    });
    let yaml = format!(
        "actions:
  - name: preferred
    command: echo preferred
    when: command_available({command_name:?})
  - name: fallback
    command: echo fallback
    when: not command_available({command_name:?})"
    );
    let mut doc: ManifestValue = serde_saphyr::from_str(&yaml)?;
    expand_foreach(&mut doc, &env)?;
    let actions = actions(&doc)?;
    anyhow::ensure!(actions.len() == 1, "expected exactly one action branch");
    let map = actions
        .first()
        .and_then(ManifestValue::as_object)
        .context("action map")?;
    let name = map
        .get("name")
        .and_then(ManifestValue::as_str)
        .context("action name")?;
    anyhow::ensure!(name == expected_name, "unexpected action branch: {name}");
    anyhow::ensure!(
        !map.contains_key("when"),
        "when should be removed after action expansion"
    );
    Ok(())
}

#[test]
fn expand_foreach_preserves_object_key_order() -> Result<()> {
    let env = Environment::new();
    let yaml = r"targets:
  - name: literal
    vars:
      existing: keep
    foreach:
      - 1
      - 2
    when: 'true'
    after: done
";
    let mut doc: ManifestValue = serde_saphyr::from_str(yaml)?;
    expand_foreach(&mut doc, &env)?;
    let targets = targets(&doc)?;
    anyhow::ensure!(targets.len() == 2, "expected expanded targets");
    for target in targets {
        let map = target.as_object().context("target object")?;
        let keys: Vec<&str> = map.keys().map(String::as_str).collect();
        anyhow::ensure!(
            keys == ["name", "vars", "after"],
            "key order should remain stable: {:?}",
            keys
        );
    }
    Ok(())
}

#[rstest]
#[case("false", 0, "expression false drops target")]
#[case("0", 0, "expression 0 drops target")]
#[case("true", 1, "expression true keeps target")]
#[case("1 == 1", 1, "expression equality keeps target")]
#[case("{{ 0 }}", 0, "template 0 drops target")]
#[case("{{ 1 }}", 1, "template 1 keeps target")]
#[case("{{ \"true\" }}", 1, "template lowercase true keeps target")]
#[case("{{ \"True\" }}", 1, "template mixed case True keeps target")]
#[case("{{ \"TRUE\" }}", 1, "template uppercase TRUE keeps target")]
#[case("{{ 2 }}", 0, "template 2 drops target (only 1 is truthy)")]
#[case("{{ \"yes\" }}", 0, "template yes drops target (only true/1 truthy)")]
fn expand_static_target_when_evaluation(
    #[case] when_expr: &str,
    #[case] expected_count: usize,
    #[case] description: &str,
) -> Result<()> {
    let env = Environment::new();
    let yaml = format!("targets:\n  - name: target\n    when: '{when_expr}'");
    let mut doc: ManifestValue = serde_saphyr::from_str(&yaml)?;
    expand_foreach(&mut doc, &env)?;
    let targets = targets(&doc)?;
    anyhow::ensure!(
        targets.len() == expected_count,
        "{description}: expected {expected_count} target(s), got {}",
        targets.len()
    );
    if expected_count == 1 {
        let target = targets.first().context("target")?;
        let map = target.as_object().context("target object")?;
        anyhow::ensure!(
            !map.contains_key("when"),
            "{description}: when field should be removed after evaluation"
        );
    }
    Ok(())
}

#[rstest]
#[case("{{ unclosed", "malformed template")]
#[case("", "empty when expression")]
#[case("   ", "whitespace-only when expression")]
fn expand_static_target_when_invalid_errors(
    #[case] when_expr: &str,
    #[case] description: &str,
) -> Result<()> {
    let env = Environment::new();
    let yaml = format!("targets:\n  - name: target\n    when: '{when_expr}'");
    let mut doc: ManifestValue = serde_saphyr::from_str(&yaml)?;
    let result = expand_foreach(&mut doc, &env);
    anyhow::ensure!(result.is_err(), "{description} should return Err");
    Ok(())
}

#[rstest]
#[case::targets("targets")]
#[case::actions("actions")]
fn expand_foreach_invalid_expression_errors_during_template_expansion(
    #[case] section: &str,
) -> Result<()> {
    let env = Environment::new();
    let yaml = format!("{section}:\n  - name: bad\n    foreach: '('");
    let mut doc: ManifestValue = serde_saphyr::from_str(&yaml)?;
    let result = expand_foreach(&mut doc, &env);
    anyhow::ensure!(
        result.is_err(),
        "invalid foreach expression should fail for {section}"
    );
    Ok(())
}
