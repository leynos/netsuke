//! Expands manifest foreach directives into concrete targets and actions.
use super::{
    ManifestMap, ManifestValue,
    budget::{ManifestBudget, ManifestBudgetStage},
};
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
    expand_foreach_with_budget(doc, env, &budget)
}

/// Expand targets and actions with the caller's shared manifest resource budget.
pub(crate) fn expand_foreach_with_budget(
    doc: &mut ManifestValue,
    env: &Environment,
    budget: &ManifestBudget,
) -> Result<ExpansionReport> {
    let mut report = ExpansionReport::default();
    for section in ["targets", "actions"] {
        let context = ExpansionContext {
            env,
            budget,
            section,
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
    let values = parse_foreach_values(expression, context)?;
    let iter = values.try_iter().context(crate::localization::message(
        crate::localization::keys::MANIFEST_FOREACH_NOT_ITERABLE,
    ))?;
    let mut expanded = Vec::new();
    for (index, item) in iter.enumerate() {
        let foreach_item = ForeachItem {
            source: map,
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
    let mut entry = foreach_item.source.clone();
    entry.remove("foreach");
    match when_allows(
        &mut entry,
        context,
        Some((foreach_item.item, foreach_item.index)),
        report.has_filtered_entry_capacity(),
    )? {
        WhenResolution::Include => {}
        WhenResolution::Exclude(event) => {
            report.record_filtered_entry(context.section == "targets", event);
            return Ok(None);
        }
        WhenResolution::Conditional => {
            entry.insert("conditional".into(), ManifestValue::Bool(true));
        }
    }
    inject_iteration_vars(&mut entry, foreach_item.item, foreach_item.index)?;
    Ok(Some(ManifestValue::Object(entry)))
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
    match when_allows(
        &mut map,
        context,
        None,
        report.has_filtered_entry_capacity(),
    )? {
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
#[path = "../expand_tests.rs"]
mod tests;
