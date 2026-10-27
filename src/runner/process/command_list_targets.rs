//! Resolve command-list marker fingerprints to bounded build-target names.

use crate::{
    ast::{Recipe, StringOrList},
    ir::{BuildEdge, BuildGraph},
    ninja_gen::command_list::action_identity,
};
use std::{collections::BTreeMap, fmt::Write as _};

/// Maximum UTF-8 bytes in one quoted display name, including its marker.
const MAX_TARGET_NAME_BYTES: usize = 64;
/// Bytes reserved for the display name's opening and closing quotes.
const TARGET_NAME_QUOTES_BYTES: usize = 2;
/// Maximum number of candidate names shown in a failure clause.
const MAX_SHOWN_CANDIDATES: usize = 3;
/// Visible suffix marking a shortened output name.
const TRUNCATION_MARKER: &str = "…";

/// Invocation-local index from generated action fingerprints to output names.
#[derive(Debug, Default)]
pub(in crate::runner) struct CommandListTargetIndex {
    /// Sorted, quoted display names for each lowercase action fingerprint.
    targets_by_fingerprint: BTreeMap<String, Vec<String>>,
}

/// One private result of resolving a marker against the invocation graph.
enum TargetResolution<'index> {
    /// Exactly one canonical command-list edge matched the fingerprint.
    Unique(&'index str),
    /// Several canonical edges matched, with a bounded displayed prefix.
    Ambiguous {
        /// Sorted candidate names shown in the diagnostic.
        shown: &'index [String],
        /// Candidates omitted from the bounded diagnostic.
        remaining: usize,
    },
    /// No command-list edge matched the fingerprint.
    NotFound,
}

impl CommandListTargetIndex {
    /// Build a bounded lookup from canonical command-list edges in `graph`.
    pub(in crate::runner) fn from_graph(graph: &BuildGraph) -> Self {
        let mut targets_by_fingerprint = BTreeMap::<String, Vec<String>>::new();
        for edge in graph.edges() {
            let Some(output) = indexable_output(graph, edge) else {
                continue;
            };

            targets_by_fingerprint
                .entry(action_identity(&edge.action_id))
                .or_default()
                .push(bound_target_name(output.as_str()));
        }

        for candidates in targets_by_fingerprint.values_mut() {
            candidates.sort();
        }
        Self {
            targets_by_fingerprint,
        }
    }

    /// Resolve a marker fingerprint without treating case as significant.
    fn resolve(&self, fingerprint: &str) -> TargetResolution<'_> {
        let key = fingerprint.to_ascii_lowercase();
        match self.targets_by_fingerprint.get(&key).map(Vec::as_slice) {
            Some([target]) => TargetResolution::Unique(target),
            Some(candidates) if candidates.len() > 1 => TargetResolution::Ambiguous {
                shown: candidates
                    .get(..candidates.len().min(MAX_SHOWN_CANDIDATES))
                    .unwrap_or_default(),
                remaining: candidates.len().saturating_sub(MAX_SHOWN_CANDIDATES),
            },
            _ => TargetResolution::NotFound,
        }
    }

    /// Format one bounded clause shared by diagnostics and tracing.
    pub(in crate::runner) fn clause(&self, fingerprint: &str) -> Option<String> {
        self.resolve(fingerprint).format_clause()
    }
}

impl TargetResolution<'_> {
    /// Format a definite target or a bounded, explicitly incomplete candidate list.
    fn format_clause(self) -> Option<String> {
        match self {
            Self::Unique(target) => Some(format!("target {target}")),
            Self::Ambiguous { shown, remaining } => {
                let shown_names = shown
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(", ");
                let mut clause = format!("targets {shown_names}");
                if remaining > 0 {
                    write!(clause, " and {remaining} more").unwrap_or(());
                }
                Some(clause)
            }
            Self::NotFound => None,
        }
    }
}

/// Return the preferred output for an indexable command-list edge.
fn indexable_output<'graph>(
    graph: &'graph BuildGraph,
    edge: &'graph BuildEdge,
) -> Option<&'graph camino::Utf8PathBuf> {
    if edge.phony {
        return None;
    }
    let action = graph.actions.get(&edge.action_id)?;
    if !is_command_list(&action.recipe) {
        return None;
    }
    first_output(edge)
}

/// Return whether `recipe` carries an ordered command-list recipe.
const fn is_command_list(recipe: &Recipe) -> bool {
    matches!(
        recipe,
        Recipe::Command {
            command: StringOrList::List(entries),
            ..
        } if !entries.is_empty()
    )
}

/// Return the preferred visible output for one canonical edge.
fn first_output(edge: &BuildEdge) -> Option<&camino::Utf8PathBuf> {
    edge.explicit_outputs
        .first()
        .or_else(|| edge.implicit_outputs.first())
}

/// Escape and bound one output name for a human-readable diagnostic.
///
/// For example, a line feed in `check-fmt` remains visible as `"check\nfmt"`.
fn bound_target_name(target: &str) -> String {
    let content_limit = MAX_TARGET_NAME_BYTES - TARGET_NAME_QUOTES_BYTES;
    let prefix_limit = content_limit - TRUNCATION_MARKER.len();
    let mut escaped = String::with_capacity(content_limit);
    let mut bounded_prefix_len = 0;

    for character in target.chars() {
        push_debug_escaped_character(&mut escaped, character);
        if escaped.len() > content_limit {
            escaped.truncate(bounded_prefix_len);
            escaped.push_str(TRUNCATION_MARKER);
            break;
        }
        if escaped.len() <= prefix_limit {
            bounded_prefix_len = escaped.len();
        }
    }
    format!("\"{escaped}\"")
}

/// Append one character using Rust string debug escaping without surrounding quotes.
fn push_debug_escaped_character(escaped: &mut String, character: char) {
    match character {
        '"' => escaped.push_str("\\\""),
        '\'' => escaped.push('\''),
        _ => escaped.extend(character.escape_debug()),
    }
}

#[cfg(test)]
#[path = "target_name_property_tests.rs"]
mod property_tests;
