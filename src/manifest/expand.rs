//! Expands manifest foreach directives into concrete targets and actions.
use super::jinja_macros::{QueryEvaluation, evaluate_when_expression, render_when_template};
use super::{ManifestMap, ManifestValue};
use crate::hex::push_lower_hex_byte;
use crate::localization::{self, keys};
use anyhow::{Context, Result};
use minijinja::{Environment, context, value::Value};
use serde_json::{Number as JsonNumber, map::Entry};
use sha2::{Digest, Sha256};

/// Counts of manifest entries excluded during template expansion.
///
/// `filtered_targets` records how many target entries were skipped because a
/// `when` condition evaluated to false. `filtered_actions` records the same
/// count for action entries, allowing callers to report or assert how much
/// manifest filtering occurred.
#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
pub(crate) struct FilteringStats {
    /// Target entries skipped because a `when` condition evaluated to false.
    pub filtered_targets: usize,
    /// Action entries skipped because a `when` condition evaluated to false.
    pub filtered_actions: usize,
}

/// A manifest entry removed by a `when` expression during expansion.
///
/// Carries only bounded, non-sensitive correlation data: the raw entry name
/// has unbounded cardinality and may carry personally identifiable
/// information, so only a short stable hash is recorded, and the raw `when`
/// expression may contain secret literals, so only its length is exposed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FilteredEntry {
    /// Manifest section the entry belonged to (`targets` or `actions`).
    pub section: String,
    /// Short stable hash of the entry name for correlation.
    pub entry_name_hash: String,
    /// Iteration index when the entry came from a `foreach` expansion.
    pub iteration_index: Option<usize>,
    /// Length of the `when` expression that filtered the entry.
    pub when_expression_len: usize,
}

/// Outcome of manifest expansion: counts plus per-entry filtering events.
///
/// Expansion reports what it filtered through this data structure rather
/// than emitting telemetry itself; the caller owns the tracing policy.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct ExpansionReport {
    /// Counts of filtered entries per section.
    pub stats: FilteringStats,
    /// One event per entry removed by a `when` expression.
    pub filtered_entries: Vec<FilteredEntry>,
}
/// Context shared by expansion operations.
///
/// `env` is the Jinja environment used to render templates. `section` is the
/// name of the manifest section currently being expanded, such as `targets` or
/// `actions`.
struct ExpansionContext<'a> {
    /// Jinja environment used to render `foreach` and `when` expressions.
    env: &'a Environment<'a>,
    /// Name of the manifest section being expanded, such as `targets`.
    section: &'a str,
}

/// Decides how manifest discovery should handle an evaluated `when` expression.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WhenEvaluation {
    /// The expression evaluated to true.
    Include,
    /// The expression evaluated to false.
    Exclude,
    /// A query-disabled helper prevents evaluation without making the entry invalid.
    Conditional,
}

/// Decides how expansion should handle a manifest entry after evaluating `when`.
#[derive(Clone, Debug, Eq, PartialEq)]
enum WhenResolution {
    /// The entry remains in the expanded manifest.
    Include,
    /// The entry is removed and supplies bounded metadata for caller-owned telemetry.
    Exclude(FilteredEntry),
    /// The entry remains conditional because a query-disabled helper prevented evaluation.
    Conditional,
}

/// Expand manifest targets and actions defined with the `foreach` key.
///
/// # Errors
///
/// Returns an error when evaluating `foreach` or `when` expressions, when
/// iteration values fail to serialize, or when target metadata is malformed.
pub(crate) fn expand_foreach(
    doc: &mut ManifestValue,
    env: &Environment,
) -> Result<ExpansionReport> {
    let filtered_targets = expand_section(doc, "targets", env)?;
    let filtered_actions = expand_section(doc, "actions", env)?;
    let stats = FilteringStats {
        filtered_targets: filtered_targets.len(),
        filtered_actions: filtered_actions.len(),
    };
    let mut filtered_entries = filtered_targets;
    filtered_entries.extend(filtered_actions);
    Ok(ExpansionReport {
        stats,
        filtered_entries,
    })
}

/// Expand one manifest section, returning metadata for each filtered entry.
fn expand_section(
    doc: &mut ManifestValue,
    key: &str,
    env: &Environment,
) -> Result<Vec<FilteredEntry>> {
    let Some(entries) = doc.get_mut(key).and_then(|v| v.as_array_mut()) else {
        return Ok(Vec::new());
    };

    let mut expanded = Vec::new();
    let mut filtered = Vec::new();
    let context = ExpansionContext { env, section: key };
    for entry in std::mem::take(entries) {
        match entry {
            ManifestValue::Object(map) => {
                expanded.extend(expand_target(map, &context, &mut filtered)?);
            }
            other => expanded.push(other),
        }
    }

    *entries = expanded;
    Ok(filtered)
}

/// Expand a single target into its concrete entries, honouring `foreach`.
fn expand_target(
    mut map: ManifestMap,
    context: &ExpansionContext<'_>,
    filtered: &mut Vec<FilteredEntry>,
) -> Result<Vec<ManifestValue>> {
    if let Some(expr_val) = map.get("foreach") {
        let values = parse_foreach_values(expr_val, context.env)?;
        let mut items = Vec::new();
        for (index, item) in values.into_iter().enumerate() {
            let mut clone = map.clone();
            clone.remove("foreach");
            match when_allows(&mut clone, context, Some((&item, index)))? {
                WhenResolution::Include => {}
                WhenResolution::Exclude(event) => {
                    filtered.push(event);
                    continue;
                }
                WhenResolution::Conditional => {
                    clone.insert("conditional".into(), ManifestValue::Bool(true));
                }
            }
            inject_iteration_vars(&mut clone, &item, index)?;
            items.push(ManifestValue::Object(clone));
        }
        Ok(items)
    } else {
        // For targets without foreach, still evaluate and remove the `when` clause.
        // Use empty context since there's no iteration variable.
        match when_allows(&mut map, context, None)? {
            WhenResolution::Include => {}
            WhenResolution::Exclude(event) => {
                filtered.push(event);
                return Ok(vec![]);
            }
            WhenResolution::Conditional => {
                map.insert("conditional".into(), ManifestValue::Bool(true));
            }
        }
        Ok(vec![ManifestValue::Object(map)])
    }
}

/// Read a target's `name`, defaulting to `<unnamed>`.
fn entry_name(map: &ManifestMap) -> &str {
    map.get("name")
        .and_then(ManifestValue::as_str)
        .unwrap_or("<unnamed>")
}

/// Derive a short stable hash of an entry name for filtered-entry logs.
fn entry_name_hash(entry_name: &str) -> String {
    let digest = Sha256::digest(entry_name.as_bytes());
    digest
        .iter()
        .take(4)
        .fold(String::with_capacity(8), |mut hash, byte| {
            push_lower_hex_byte(&mut hash, *byte);
            hash
        })
}

/// Resolve `foreach` values from an inline array or a Jinja expression.
fn parse_foreach_values(expr_val: &ManifestValue, env: &Environment) -> Result<Vec<Value>> {
    if let Some(seq) = expr_val.as_array() {
        return Ok(seq.iter().cloned().map(Value::from_serialize).collect());
    }
    let expr = as_str(expr_val, "foreach")?;
    let seq = eval_expression(env, "foreach", expr, context! {})?;
    let iter = seq
        .try_iter()
        .context(localization::message(keys::MANIFEST_FOREACH_NOT_ITERABLE))?;
    Ok(iter.collect())
}

/// Evaluate a `when` clause and return its discovery resolution.
///
/// The `when` clause can be either:
/// - A Jinja expression (e.g., `item > 1`) - evaluated via `compile_expression`
/// - A Jinja template (e.g., `{{ path is dir }}`) - evaluated via `render_str`
///
/// Detection strategy: attempt expression compilation first; if parsing fails,
/// fall back to template rendering. This avoids brittle heuristics like
/// checking for `{{` which could appear in string literals.
///
/// Empty expressions are rejected as invalid.
fn eval_when(env: &Environment, expr: &str, ctx: &Value) -> Result<WhenEvaluation> {
    anyhow::ensure!(
        !expr.trim().is_empty(),
        "{}",
        localization::message(keys::MANIFEST_WHEN_EMPTY)
    );

    // Try expression compilation first - this handles plain expressions
    // like "item > 1" or "true" without needing template delimiters.
    if let Some(evaluation) = evaluate_when_expression(env, expr, ctx)? {
        return Ok(match evaluation {
            QueryEvaluation::Value(is_true) => when_evaluation(is_true),
            QueryEvaluation::QueryDisabled => WhenEvaluation::Conditional,
        });
    }

    // Expression parsing failed - treat as template syntax (e.g., "{{ path is dir }}")
    let rendered_template = match render_when_template(env, expr, ctx)? {
        QueryEvaluation::Value(output) => output,
        QueryEvaluation::QueryDisabled => return Ok(WhenEvaluation::Conditional),
    };
    // Treat "true" or "1" as truthy, anything else (including "false", "") as falsy
    Ok(when_evaluation(matches!(
        rendered_template.trim().to_lowercase().as_str(),
        "true" | "1"
    )))
}

/// Map a successfully evaluated boolean condition to its discovery resolution.
const fn when_evaluation(is_true: bool) -> WhenEvaluation {
    if is_true {
        WhenEvaluation::Include
    } else {
        WhenEvaluation::Exclude
    }
}

/// Evaluate a `when` clause and return the entry's expansion outcome.
///
/// Accepts an optional iteration context (`item`, `index`) for foreach targets;
/// static targets pass `None`.
fn when_allows(
    map: &mut ManifestMap,
    context: &ExpansionContext<'_>,
    iteration: Option<(&Value, usize)>,
) -> Result<WhenResolution> {
    let Some(when_val) = map.remove("when") else {
        return Ok(WhenResolution::Include);
    };
    let expr = as_str(&when_val, "when")?;
    let ctx = when_context(map, iteration)?;
    match eval_when(context.env, expr, &ctx)? {
        WhenEvaluation::Include => Ok(WhenResolution::Include),
        WhenEvaluation::Conditional => Ok(WhenResolution::Conditional),
        WhenEvaluation::Exclude => Ok(WhenResolution::Exclude(FilteredEntry {
            section: context.section.to_owned(),
            entry_name_hash: entry_name_hash(entry_name(map)),
            iteration_index: iteration.map(|(_, index)| index),
            when_expression_len: expr.len(),
        })),
    }
}
/// Build the Jinja context for a `when` condition, adding `item` and `index`.
fn when_context(map: &ManifestMap, iteration: Option<(&Value, usize)>) -> Result<Value> {
    let mut vars = map
        .get("vars")
        .and_then(ManifestValue::as_object)
        .cloned()
        .unwrap_or_default();
    if let Some((item, index)) = iteration {
        vars.insert(
            "item".into(),
            serde_json::to_value(item)
                .context(localization::message(keys::MANIFEST_FOREACH_SERIALISE_ITEM))?,
        );
        vars.insert(
            "index".into(),
            ManifestValue::Number(JsonNumber::from(index as u64)),
        );
    }
    Ok(Value::from_serialize(vars))
}

/// Inject `item` and `index` into a target's `vars`, creating the map when absent.
fn inject_iteration_vars(map: &mut ManifestMap, item: &Value, index: usize) -> Result<()> {
    let vars_value = match map.entry("vars") {
        Entry::Vacant(slot) => slot.insert(ManifestValue::Object(ManifestMap::new())),
        Entry::Occupied(slot) => {
            let value = slot.into_mut();
            match value {
                ManifestValue::Object(_) => value,
                other => {
                    return Err(anyhow::anyhow!(
                        "{}",
                        localization::message(keys::MANIFEST_TARGET_VARS_NOT_OBJECT)
                            .with_arg("value", format!("{other:?}"))
                    ));
                }
            }
        }
    };

    let vars = vars_value.as_object_mut().ok_or_else(|| {
        anyhow::anyhow!(
            "{}",
            localization::message(keys::MANIFEST_VARS_ENTRY_NOT_OBJECT)
        )
    })?;
    vars.insert(
        "item".into(),
        serde_json::to_value(item)
            .context(localization::message(keys::MANIFEST_FOREACH_SERIALISE_ITEM))?,
    );
    let index_value = ManifestValue::Number(JsonNumber::from(index as u64));
    vars.insert("index".into(), index_value);
    Ok(())
}

/// Extract a manifest value as a string, erroring when it is not one.
fn as_str<'a>(value: &'a ManifestValue, field: &str) -> Result<&'a str> {
    value.as_str().ok_or_else(|| {
        anyhow::anyhow!(
            "{}",
            localization::message(keys::MANIFEST_FIELD_NOT_STRING).with_arg("field", field)
        )
    })
}

/// Evaluate a Jinja expression, mapping parse and evaluation errors.
fn eval_expression(env: &Environment, name: &str, expr: &str, ctx: Value) -> Result<Value> {
    env.compile_expression(expr)
        .with_context(|| {
            localization::message(keys::MANIFEST_EXPRESSION_PARSE_ERROR).with_arg("name", name)
        })?
        .eval(ctx)
        .with_context(|| {
            localization::message(keys::MANIFEST_EXPRESSION_EVAL_ERROR).with_arg("name", name)
        })
}

#[cfg(test)]
#[path = "expand_tests.rs"]
mod tests;
