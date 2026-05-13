//! Unit tests for manifest foreach expansion.

use super::*;
use minijinja::Environment;
use rstest::rstest;
use std::sync::{Arc, Mutex, OnceLock};
use tracing::{
    Event, Subscriber,
    field::{Field, Visit},
};
use tracing_subscriber::{Layer, layer::Context as LayerContext, prelude::*, registry::LookupSpan};

#[derive(Debug, Clone, Default)]
struct CapturedEvents {
    fields: Arc<Mutex<Vec<String>>>,
}

impl CapturedEvents {
    fn snapshot(&self) -> Vec<String> {
        self.fields.lock().expect("captured events lock").clone()
    }
}

impl<S> Layer<S> for CapturedEvents
where
    S: Subscriber + for<'span> LookupSpan<'span>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: LayerContext<'_, S>) {
        let mut visitor = FieldVisitor::default();
        event.record(&mut visitor);
        self.fields
            .lock()
            .expect("captured events lock")
            .push(visitor.fields.join(" "));
    }
}

#[derive(Debug, Default)]
struct FieldVisitor {
    fields: Vec<String>,
}

impl Visit for FieldVisitor {
    fn record_bool(&mut self, field: &Field, value: bool) {
        self.fields.push(format!("{}={value}", field.name()));
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.fields.push(format!("{}={value}", field.name()));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.fields.push(format!("{}={value}", field.name()));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.fields.push(format!("{}={value:?}", field.name()));
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.fields.push(format!("{}={value:?}", field.name()));
    }
}

fn targets(doc: &ManifestValue) -> Result<&[ManifestValue]> {
    doc.get("targets")
        .and_then(|v| v.as_array())
        .map(Vec::as_slice)
        .context("targets sequence missing")
}

fn actions(doc: &ManifestValue) -> Result<&[ManifestValue]> {
    doc.get("actions")
        .and_then(|v| v.as_array())
        .map(Vec::as_slice)
        .context("actions sequence missing")
}

fn section_entries<'a>(doc: &'a ManifestValue, section: &str) -> Result<&'a [ManifestValue]> {
    doc.get(section)
        .and_then(|v| v.as_array())
        .map(Vec::as_slice)
        .with_context(|| format!("{section} sequence missing"))
}

fn indexes(entries: &[ManifestValue], section: &str) -> Result<Vec<u64>> {
    entries
        .iter()
        .map(|entry| -> Result<u64> {
            let map = entry
                .as_object()
                .with_context(|| format!("{section} entry map"))?;
            let vars = map
                .get("vars")
                .and_then(|v| v.as_object())
                .with_context(|| format!("{section} vars map"))?;
            let index_value = vars
                .get("index")
                .with_context(|| format!("{section} index value"))?;
            let ManifestValue::Number(num) = index_value else {
                anyhow::bail!("{section} index missing");
            };
            num.as_u64()
                .with_context(|| format!("{section} numeric index conversion failed"))
        })
        .collect()
}

#[test]
fn expand_foreach_returns_filtering_stats() -> Result<()> {
    let env = Environment::new();
    let yaml = "targets:
  - name: skipped-target
    command: echo skipped
    when: 'false'
  - name: kept-target
    command: echo kept
actions:
  - name: skipped-action
    command: echo skipped
    when: 'false'
  - name: each-action
    command: echo {{ item }}
    foreach:
      - skip
      - keep
    when: item != 'skip'";
    let mut doc: ManifestValue = serde_saphyr::from_str(yaml)?;

    let stats = expand_foreach(&mut doc, &env)?;

    anyhow::ensure!(
        stats
            == FilteringStats {
                filtered_targets: 1,
                filtered_actions: 2,
            },
        "unexpected filtering stats: {stats:?}"
    );
    anyhow::ensure!(targets(&doc)?.len() == 1, "expected one kept target");
    anyhow::ensure!(actions(&doc)?.len() == 1, "expected one kept action");
    Ok(())
}

#[test]
fn expand_foreach_emits_debug_event_for_filtered_entry() -> Result<()> {
    let env = Environment::new();
    let when_expr = "secret_token == 'literal-secret'";
    let yaml = format!(
        "targets:
  - name: skipped-target-secret
    command: echo skipped
    when: {when_expr}"
    );
    let mut doc: ManifestValue = serde_saphyr::from_str(&yaml)?;
    let captured = installed_test_subscriber();
    let start_index = captured.snapshot().len();
    tracing::callsite::rebuild_interest_cache();

    let stats = expand_foreach(&mut doc, &env)?;
    let snapshot = captured.snapshot();
    let events = snapshot
        .get(start_index..)
        .context("captured event start index")?
        .to_vec();

    anyhow::ensure!(
        stats.filtered_targets == 1,
        "expected one filtered target: {stats:?}"
    );
    let expected_hash = entry_name_hash("skipped-target-secret");
    let event = events
        .iter()
        .find(|event| {
            event.contains("filtered manifest entry by when expression")
                && event.contains("section=\"targets\"")
                && event.contains(&format!("entry_name_hash=\"{expected_hash}\""))
        })
        .with_context(|| format!("expected filtered-entry debug event in {events:?}"))?;
    anyhow::ensure!(
        event.contains("section=\"targets\""),
        "debug event should include section field: {event}"
    );
    anyhow::ensure!(
        event.contains(&format!("entry_name_hash=\"{expected_hash}\"")),
        "debug event should include bounded entry name hash: {event}"
    );
    anyhow::ensure!(
        event.contains(&format!("when_expression_len={}", when_expr.len())),
        "debug event should include expression length: {event}"
    );
    anyhow::ensure!(
        event.contains("when_result=false"),
        "debug event should include false when result: {event}"
    );
    anyhow::ensure!(
        !event.contains("skipped-target-secret"),
        "debug event should not include raw entry name: {event}"
    );
    anyhow::ensure!(
        !event.contains("secret_token") && !event.contains("literal-secret"),
        "debug event should not include raw when expression: {event}"
    );
    Ok(())
}

fn installed_test_subscriber() -> CapturedEvents {
    static CAPTURED: OnceLock<CapturedEvents> = OnceLock::new();
    CAPTURED
        .get_or_init(|| {
            let captured = CapturedEvents::default();
            let subscriber = tracing_subscriber::registry().with(captured.clone());
            tracing::subscriber::set_global_default(subscriber)
                .expect("install manifest expansion test subscriber");
            captured
        })
        .clone()
}

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
