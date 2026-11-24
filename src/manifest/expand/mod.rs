//! Expands manifest foreach directives into concrete targets and actions.
use super::{
    ManifestMap, ManifestValue,
    budget::{ManifestBudget, ManifestBudgetStage, retained_size},
};
use crate::manifest::budget::adapter::BudgetErrorExt;
use anyhow::{Context, Result};
use minijinja::Environment;

mod evaluation;
#[cfg(test)]
use evaluation::entry_name_hash;
use evaluation::{inject_iteration_vars, parse_foreach_values, when_allows};

/// Limit the number of filtered-entry records retained for telemetry.
const FILTERED_ENTRY_RETENTION_LIMIT: usize = 64;

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
    /// Bounded per-entry records for entries removed by a `when` expression.
    pub filtered_entries: Vec<FilteredEntry>,
    /// Number of filtered entries not retained in `filtered_entries`.
    pub omitted_filtered_entries: usize,
}

impl ExpansionReport {
    /// Record a filtered entry while preserving exact aggregate counts.
    fn record_filtered_entry(&mut self, is_target: bool, filtered_entry: Option<FilteredEntry>) {
        if is_target {
            self.stats.filtered_targets += 1;
        } else {
            self.stats.filtered_actions += 1;
        }
        if let Some(retained_entry) = filtered_entry {
            self.filtered_entries.push(retained_entry);
        } else {
            self.omitted_filtered_entries += 1;
        }
    }

    /// Report whether another filtered-entry record can be retained.
    const fn has_filtered_entry_capacity(&self) -> bool {
        self.filtered_entries.len() < FILTERED_ENTRY_RETENTION_LIMIT
    }
}
/// Context shared by expansion operations.
///
/// `env` is the Jinja environment used to render templates. `section` is the
/// name of the manifest section currently being expanded, such as `targets` or
/// `actions`.
struct ExpansionContext<'a> {
    /// Jinja environment used to render `foreach` and `when` expressions.
    env: &'a Environment<'a>,
    /// Shared resource accounting for this complete manifest evaluation.
    budget: &'a ManifestBudget,
    /// Name of the manifest section being expanded, such as `targets`.
    section: &'a str,
    /// Whether a refused helper marks the entry conditional.
    classifies_query_refusals: bool,
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
    /// The entry is removed and supplies metadata only while report capacity remains.
    Exclude(Option<FilteredEntry>),
    /// The entry remains conditional because a query-disabled helper prevented evaluation.
    Conditional,
}

/// Expand manifest targets and actions defined with the `foreach` key.
///
/// # Errors
///
/// Returns an error when evaluating `foreach` or `when` expressions, when
/// iteration values fail to serialize, or when target metadata is malformed.
#[cfg(test)]
pub(crate) fn expand_foreach(
    doc: &mut ManifestValue,
    env: &Environment,
) -> Result<ExpansionReport> {
    let budget = ManifestBudget::default();
    // The suite builds both complete and manifest-query environments here, and
    // only a query surface can emit the refusal marker, so classifying it is
    // inert for the complete ones.
    expand_foreach_with_budget(doc, env, &budget, true)
}

/// Expand targets and actions with the caller's shared manifest resource budget.
///
/// `classifies_query_refusals` comes from the load's registration: a discovery
/// query turns a refused helper into a conditional entry, while a full load
/// propagates it as a failure.
pub(crate) fn expand_foreach_with_budget(
    doc: &mut ManifestValue,
    env: &Environment,
    budget: &ManifestBudget,
    classifies_query_refusals: bool,
) -> Result<ExpansionReport> {
    let mut report = ExpansionReport::default();
    for section in ["targets", "actions"] {
        let context = ExpansionContext {
            env,
            budget,
            section,
            classifies_query_refusals,
        };
        expand_section(doc, &context, &mut report)?;
    }
    Ok(report)
}

/// Expand one manifest section and record any filtered entries in `report`.
fn expand_section(
    doc: &mut ManifestValue,
    context: &ExpansionContext<'_>,
    report: &mut ExpansionReport,
) -> Result<()> {
    let Some(entries) = doc.get_mut(context.section).and_then(|v| v.as_array_mut()) else {
        return Ok(());
    };

    let mut expanded = Vec::new();
    for entry in std::mem::take(entries) {
        match entry {
            ManifestValue::Object(map) => {
                expanded.extend(expand_target(map, context, report)?);
            }
            other => expanded.push(other),
        }
    }

    *entries = expanded;
    Ok(())
}

/// Expand a single target into its concrete entries, honouring `foreach`.
fn expand_target(
    map: ManifestMap,
    context: &ExpansionContext<'_>,
    report: &mut ExpansionReport,
) -> Result<Vec<ManifestValue>> {
    if map.contains_key("foreach") {
        expand_foreach_target(&map, context, report)
    } else {
        expand_static_target(map, context, report)
    }
}

/// Expand one entry's `foreach` iterator without materialising it first.
fn expand_foreach_target(
    map: &ManifestMap,
    context: &ExpansionContext<'_>,
    report: &mut ExpansionReport,
) -> Result<Vec<ManifestValue>> {
    let Some(expression) = map.get("foreach") else {
        return Ok(Vec::new());
    };
    // Every admitted item retains one clone of this source, so its structural
    // size is charged per item. Estimate it once, bounded by the configured
    // limit. The refusal itself waits for the first item: an empty iterator
    // clones nothing, so a large source is harmless there.
    let source_bytes = retained_size::source_map_bytes(map, context.budget.expanded_bytes_limit());
    let values = parse_foreach_values(expression, context)?;
    let iter = values.try_iter().context(crate::localization::message(
        crate::localization::keys::MANIFEST_FOREACH_NOT_ITERABLE,
    ))?;
    let mut expanded = Vec::new();
    for (index, item) in iter.enumerate() {
        let foreach_item = ForeachItem {
            source: map,
            source_bytes,
            item: &item,
            index,
        };
        if let Some(entry) = expand_foreach_item(&foreach_item, context, report)? {
            expanded.push(entry);
        }
    }
    Ok(expanded)
}

/// Borrow the source and iterator state needed to expand one `foreach` item.
struct ForeachItem<'a> {
    /// Holds the target or action mapping to clone after its budget charge.
    source: &'a ManifestMap,
    /// Holds the modelled bytes retained by one source clone, or `None` when
    /// the source alone cannot fit any admitted item.
    source_bytes: Option<usize>,
    /// Holds the current lazily consumed iterator item.
    item: &'a minijinja::value::Value,
    /// Identifies the zero-based iterator position.
    index: usize,
}

/// Expand one lazy `foreach` item after charging its bounded resource costs.
fn expand_foreach_item(
    foreach_item: &ForeachItem<'_>,
    context: &ExpansionContext<'_>,
    report: &mut ExpansionReport,
) -> Result<Option<ManifestValue>> {
    context
        .budget
        .check_foreach_cardinality(foreach_item.index)
        .map_err(|exhaustion| exhaustion.into_error(minijinja::ErrorKind::InvalidOperation))?;
    context
        .budget
        .charge_expanded_entry(ManifestBudgetStage::ExpansionAggregate)
        .map_err(|exhaustion| exhaustion.into_error(minijinja::ErrorKind::InvalidOperation))?;
    // A source that cannot fit any admitted clone is refused here, once an
    // item exists; an empty iterator never reaches this point.
    let source_bytes = foreach_item
        .source_bytes
        .ok_or_else(|| {
            context
                .budget
                .expanded_bytes_exhaustion(ManifestBudgetStage::ExpansionAggregate)
        })
        .map_err(|exhaustion| exhaustion.into_error(minijinja::ErrorKind::InvalidOperation))?;
    // Refuse an item that cannot fit the remaining allowance before `when`
    // copies it into a temporary condition context.
    let item_bytes = retained_size::item_value_bytes(
        foreach_item.item,
        context.budget.remaining_expanded_bytes(),
    )
    .ok_or_else(|| {
        context
            .budget
            .expanded_bytes_exhaustion(ManifestBudgetStage::ExpansionAggregate)
    })
    .map_err(|exhaustion| exhaustion.into_error(minijinja::ErrorKind::InvalidOperation))?;
    let resolution = when_allows(
        foreach_item.source,
        context,
        Some((foreach_item.item, foreach_item.index)),
        report.has_filtered_entry_capacity(),
    )?;
    let is_conditional = match resolution {
        WhenResolution::Include => false,
        WhenResolution::Exclude(event) => {
            report.record_filtered_entry(context.section == "targets", event);
            return Ok(None);
        }
        WhenResolution::Conditional => true,
    };
    charge_retained_bytes(source_bytes, item_bytes, context)?;
    let mut entry = clone_expansion_entry(foreach_item.source);
    if is_conditional {
        entry.insert("conditional".into(), ManifestValue::Bool(true));
    }
    let serialized_bytes =
        inject_iteration_vars(&mut entry, foreach_item.item, foreach_item.index)?;
    charge_serialized_growth(context, item_bytes, serialized_bytes)?;
    Ok(Some(ManifestValue::Object(entry)))
}

/// Charge the source, item, and injected-key bytes retained by one expansion.
fn charge_retained_bytes(
    source_bytes: usize,
    item_bytes: usize,
    context: &ExpansionContext<'_>,
) -> Result<()> {
    let retained_bytes = source_bytes
        .checked_add(item_bytes)
        .and_then(|total| total.checked_add(retained_size::iteration_overhead()))
        .ok_or_else(|| {
            context
                .budget
                .expanded_bytes_exhaustion(ManifestBudgetStage::ExpansionAggregate)
        })
        .map_err(|exhaustion| exhaustion.into_error(minijinja::ErrorKind::InvalidOperation))?;
    context
        .budget
        .charge_expanded_bytes(ManifestBudgetStage::ExpansionAggregate, retained_bytes)
        .map_err(|exhaustion| exhaustion.into_error(minijinja::ErrorKind::InvalidOperation))?;
    Ok(())
}

/// Charge modelled serialization growth beyond the item's earlier estimate.
fn charge_serialized_growth(
    context: &ExpansionContext<'_>,
    item_bytes: usize,
    serialized_bytes: usize,
) -> Result<()> {
    if serialized_bytes <= item_bytes {
        return Ok(());
    }
    context
        .budget
        .charge_expanded_bytes(
            ManifestBudgetStage::ExpansionAggregate,
            serialized_bytes - item_bytes,
        )
        .map_err(|exhaustion| exhaustion.into_error(minijinja::ErrorKind::InvalidOperation))?;
    Ok(())
}

/// Clone one source entry while dropping keys expansion itself consumes.
fn clone_expansion_entry(source: &ManifestMap) -> ManifestMap {
    source
        .iter()
        .filter(|(key, _)| !retained_size::is_expansion_local_key(key))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

/// Expand a target without `foreach` after evaluating its optional `when` clause.
fn expand_static_target(
    mut map: ManifestMap,
    context: &ExpansionContext<'_>,
    report: &mut ExpansionReport,
) -> Result<Vec<ManifestValue>> {
    context
        .budget
        .charge_expanded_entry(ManifestBudgetStage::ExpansionAggregate)
        .map_err(|exhaustion| exhaustion.into_error(minijinja::ErrorKind::InvalidOperation))?;
    let resolution = when_allows(&map, context, None, report.has_filtered_entry_capacity())?;
    drop(map.remove("when"));
    match resolution {
        WhenResolution::Include => Ok(vec![ManifestValue::Object(map)]),
        WhenResolution::Exclude(event) => {
            report.record_filtered_entry(context.section == "targets", event);
            Ok(Vec::new())
        }
        WhenResolution::Conditional => {
            map.insert("conditional".into(), ManifestValue::Bool(true));
            Ok(vec![ManifestValue::Object(map)])
        }
    }
}

#[cfg(test)]
mod tests;
