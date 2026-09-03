//! Evaluates bounded `foreach` and `when` expansion expressions.

use super::{
    ExpansionContext, FilteredEntry, ManifestMap, ManifestValue, WhenEvaluation, WhenResolution,
};
use crate::{
    hex::push_lower_hex_byte,
    localization::{self, keys},
    manifest::budget::ManifestBudgetStage,
};
use anyhow::{Context, Result};
use minijinja::{context, value::Value};
use serde_json::{Number as JsonNumber, map::Entry};
use sha2::{Digest, Sha256};

/// Resolve `foreach` values from an inline array or a Jinja expression.
pub(super) fn parse_foreach_values(
    expr_val: &ManifestValue,
    context: &ExpansionContext<'_>,
) -> Result<Value> {
    if let Some(seq) = expr_val.as_array() {
        return Ok(Value::from_serialize(seq));
    }
    let expr = as_str(expr_val, "foreach")?;
    eval_expression(
        context,
        ExpressionRequest {
            name: "foreach",
            expression: expr,
            value: context! {},
            stage: ManifestBudgetStage::Foreach,
        },
    )
}

/// Evaluate a `when` clause and return the entry's expansion outcome.
pub(super) fn when_allows(
    map: &mut ManifestMap,
    context: &ExpansionContext<'_>,
    iteration: Option<(&Value, usize)>,
    retain_filtered_entry: bool,
) -> Result<WhenResolution> {
    let Some(when_val) = map.remove("when") else {
        return Ok(WhenResolution::Include);
    };
    let expr = as_str(&when_val, "when")?;
    let expression_context = when_context(map, iteration)?;
    match eval_when(context, expr, &expression_context)? {
        WhenEvaluation::Include => Ok(WhenResolution::Include),
        WhenEvaluation::Conditional => Ok(WhenResolution::Conditional),
        WhenEvaluation::Exclude => Ok(WhenResolution::Exclude(retain_filtered_entry.then(|| {
            FilteredEntry {
                section: context.section.to_owned(),
                entry_name_hash: entry_name_hash(entry_name(map)),
                iteration_index: iteration.map(|(_, index)| index),
                when_expression_len: expr.len(),
            }
        }))),
    }
}

/// Inject `item` and `index` into a target's `vars`, creating the map when absent.
pub(super) fn inject_iteration_vars(
    map: &mut ManifestMap,
    item: &Value,
    index: usize,
) -> Result<()> {
    let vars_value = match map.entry("vars") {
        Entry::Vacant(slot) => slot.insert(ManifestValue::Object(ManifestMap::new())),
        Entry::Occupied(slot) => match slot.into_mut() {
            value @ ManifestValue::Object(_) => value,
            other => {
                return Err(anyhow::anyhow!(
                    "{}",
                    localization::message(keys::MANIFEST_TARGET_VARS_NOT_OBJECT)
                        .with_arg("value", format!("{other:?}"))
                ));
            }
        },
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
    vars.insert(
        "index".into(),
        ManifestValue::Number(JsonNumber::from(index as u64)),
    );
    Ok(())
}

/// Evaluate a `when` clause as an expression or a bounded template.
fn eval_when(
    expansion: &ExpansionContext<'_>,
    expr: &str,
    context: &Value,
) -> Result<WhenEvaluation> {
    anyhow::ensure!(
        !expr.trim().is_empty(),
        "{}",
        localization::message(keys::MANIFEST_WHEN_EMPTY)
    );
    if let Some(evaluation) = super::super::jinja_macros::evaluate_when_expression(
        expansion.env,
        expr,
        context,
        expansion.budget,
    )? {
        return Ok(match evaluation {
            super::super::jinja_macros::QueryEvaluation::Value(is_true) => when_evaluation(is_true),
            super::super::jinja_macros::QueryEvaluation::QueryDisabled => {
                WhenEvaluation::Conditional
            }
        });
    }
    let rendered = match super::super::jinja_macros::render_when_template(
        expansion.env,
        expr,
        context,
        expansion.budget,
    )? {
        super::super::jinja_macros::QueryEvaluation::Value(output) => output,
        super::super::jinja_macros::QueryEvaluation::QueryDisabled => {
            return Ok(WhenEvaluation::Conditional);
        }
    };
    Ok(when_evaluation(matches!(
        rendered.trim().to_lowercase().as_str(),
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

/// Read a target's `name`, defaulting to `<unnamed>`.
fn entry_name(map: &ManifestMap) -> &str {
    map.get("name")
        .and_then(ManifestValue::as_str)
        .unwrap_or("<unnamed>")
}

/// Derive a short stable hash of an entry name for filtered-entry logs.
pub(super) fn entry_name_hash(entry_name: &str) -> String {
    Sha256::digest(entry_name.as_bytes()).iter().take(4).fold(
        String::with_capacity(8),
        |mut hash, byte| {
            push_lower_hex_byte(&mut hash, *byte);
            hash
        },
    )
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

/// Borrow one expression evaluation request.
struct ExpressionRequest<'a> {
    /// Names the evaluation boundary in stable diagnostics.
    name: &'a str,
    /// Holds the expression source.
    expression: &'a str,
    /// Supplies expression-local template values.
    value: Value,
    /// Identifies the fixed budget stage.
    stage: ManifestBudgetStage,
}

/// Evaluate a Jinja expression, mapping parse and evaluation errors.
fn eval_expression(
    context: &ExpansionContext<'_>,
    request: ExpressionRequest<'_>,
) -> Result<Value> {
    context
        .budget
        .charge_source(request.expression.len(), ManifestBudgetStage::Source)
        .map_err(|exhaustion| exhaustion.into_error(minijinja::ErrorKind::InvalidOperation))?;
    let fuel = context
        .budget
        .reserve_fuel(request.stage)
        .map_err(|exhaustion| exhaustion.into_error(minijinja::ErrorKind::OutOfFuel))?;
    let mut bounded_env = context.env.clone();
    bounded_env.set_fuel(Some(fuel));
    bounded_env
        .compile_expression(request.expression)
        .with_context(|| {
            localization::message(keys::MANIFEST_EXPRESSION_PARSE_ERROR)
                .with_arg("name", request.name)
        })?
        .eval(request.value)
        .map_err(|error| {
            if error.kind() == minijinja::ErrorKind::OutOfFuel {
                context
                    .budget
                    .fuel_exhaustion(request.stage)
                    .into_error(minijinja::ErrorKind::OutOfFuel)
            } else {
                error
            }
        })
        .with_context(|| {
            localization::message(keys::MANIFEST_EXPRESSION_EVAL_ERROR)
                .with_arg("name", request.name)
        })
}
