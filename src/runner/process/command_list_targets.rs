//! Resolve command-list marker fingerprints to bounded build-target names.

use crate::{
    ast::{Recipe, StringOrList},
    ir::{BuildEdge, BuildGraph},
    ninja_gen::command_list::action_identity,
};
use std::{collections::BTreeMap, fmt::Write as _};

/// Maximum UTF-8 bytes retained from one output name, including its marker.
const MAX_TARGET_NAME_BYTES: usize = 64;
/// Maximum number of candidate names shown in a failure clause.
const MAX_SHOWN_CANDIDATES: usize = 3;
/// Visible suffix marking a shortened output name.
const TRUNCATION_MARKER: &str = "…";

/// Invocation-local index from generated action fingerprints to output names.
#[derive(Debug, Default)]
pub(in crate::runner) struct CommandListTargetIndex {
    /// Sorted, bounded output names for each lowercase action fingerprint.
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
            if edge.phony {
                continue;
            }
            let Some(action) = graph.actions.get(&edge.action_id) else {
                continue;
            };
            if !is_command_list(&action.recipe) {
                continue;
            }
            let Some(output) = first_output(edge) else {
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
            Self::Unique(target) => Some(format!("target {target:?}")),
            Self::Ambiguous { shown, remaining } => {
                let shown_names = shown
                    .iter()
                    .map(|target| format!("{target:?}"))
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

/// Bound one output name on a UTF-8 boundary and preserve a visible suffix.
fn bound_target_name(target: &str) -> String {
    if target.len() <= MAX_TARGET_NAME_BYTES {
        return target.to_owned();
    }

    let prefix_limit = MAX_TARGET_NAME_BYTES - TRUNCATION_MARKER.len();
    let mut boundary = prefix_limit;
    while !target.is_char_boundary(boundary) {
        boundary -= 1;
    }
    format!(
        "{}{TRUNCATION_MARKER}",
        target.get(..boundary).unwrap_or_default()
    )
}

#[cfg(test)]
mod tests {
    //! Verify bounded command-list target indexing and marker-key parity.

    use super::*;
    use crate::{
        ast::CommandIsolation,
        ir::{Action, BuildEdge, BuildGraph, DependencyOrder},
        ninja_gen::{command_list::COMMAND_LIST_FAILURE_PREFIX, generate_bundle},
    };
    use camino::Utf8PathBuf;

    /// Return a graph action that renders one command-list entry.
    fn command_list_action() -> Action {
        Action {
            recipe: Recipe::Command {
                command: StringOrList::List(vec!["true".to_owned()]),
                isolation: CommandIsolation::Shared,
            },
            description: None,
            depfile: None,
            deps_format: None,
            pool: None,
            restat: false,
        }
    }

    /// Insert one canonical edge with the supplied explicit outputs.
    fn insert_edge(
        graph: &mut BuildGraph,
        action_id: &str,
        explicit_outputs: &[&str],
        phony: bool,
    ) {
        let result = graph.insert_edge(BuildEdge {
            action_id: action_id.to_owned(),
            inputs: Vec::new(),
            implicit_deps: Vec::new(),
            dependency_order: DependencyOrder::Parallel,
            explicit_outputs: explicit_outputs.iter().map(Utf8PathBuf::from).collect(),
            implicit_outputs: Vec::new(),
            order_only_deps: Vec::new(),
            phony,
            always: false,
        });
        assert!(result.is_ok(), "test edge outputs should be unique");
    }

    /// Build an index with one command-list edge for a target.
    fn index_for_target(target: &str) -> CommandListTargetIndex {
        let mut graph = BuildGraph::default();
        graph
            .actions
            .insert("identity".to_owned(), command_list_action());
        insert_edge(&mut graph, "identity", &[target], false);
        CommandListTargetIndex::from_graph(&graph)
    }

    #[test]
    fn marker_fingerprint_matches_the_generator_key() {
        let mut graph = BuildGraph::default();
        graph
            .actions
            .insert("identity".to_owned(), command_list_action());
        insert_edge(&mut graph, "identity", &["check-fmt"], false);
        let index = CommandListTargetIndex::from_graph(&graph);
        let generated = generate_bundle(&graph).expect("command list should generate");
        let fingerprint = action_identity("identity");
        let marker = format!("{COMMAND_LIST_FAILURE_PREFIX}{fingerprint}, entry 1");

        assert!(generated.build_file().contains(&marker));
        assert_eq!(
            index.clause(&fingerprint).as_deref(),
            Some("target \"check-fmt\"")
        );
    }

    #[test]
    fn one_canonical_edge_with_multiple_outputs_is_one_candidate() {
        let mut graph = BuildGraph::default();
        graph
            .actions
            .insert("identity".to_owned(), command_list_action());
        insert_edge(&mut graph, "identity", &["first", "second"], false);
        let index = CommandListTargetIndex::from_graph(&graph);

        assert_eq!(
            index.clause(&action_identity("identity")).as_deref(),
            Some("target \"first\"")
        );
    }

    #[test]
    fn implicit_output_is_used_when_an_edge_has_no_explicit_output() {
        let mut graph = BuildGraph::default();
        graph
            .actions
            .insert("identity".to_owned(), command_list_action());
        let result = graph.insert_edge(BuildEdge {
            action_id: "identity".to_owned(),
            inputs: Vec::new(),
            implicit_deps: Vec::new(),
            dependency_order: DependencyOrder::Parallel,
            explicit_outputs: Vec::new(),
            implicit_outputs: vec![Utf8PathBuf::from("implicit-target")],
            order_only_deps: Vec::new(),
            phony: false,
            always: false,
        });
        assert!(result.is_ok(), "test edge outputs should be unique");

        assert_eq!(
            CommandListTargetIndex::from_graph(&graph)
                .clause(&action_identity("identity"))
                .as_deref(),
            Some("target \"implicit-target\"")
        );
    }

    #[test]
    fn edges_without_outputs_are_not_candidates() {
        let mut graph = BuildGraph::default();
        graph
            .actions
            .insert("identity".to_owned(), command_list_action());
        insert_edge(&mut graph, "identity", &[], false);

        assert_eq!(
            CommandListTargetIndex::from_graph(&graph).clause(&action_identity("identity")),
            None
        );
    }

    #[test]
    fn identical_direct_actions_resolve_to_sorted_ambiguous_candidates() {
        let mut graph = BuildGraph::default();
        graph
            .actions
            .insert("identity".to_owned(), command_list_action());
        insert_edge(&mut graph, "identity", &["lint"], false);
        insert_edge(&mut graph, "identity", &["check-fmt"], false);
        let index = CommandListTargetIndex::from_graph(&graph);

        assert_eq!(
            index.clause(&action_identity("identity")).as_deref(),
            Some("targets \"check-fmt\", \"lint\"")
        );
    }

    #[test]
    fn candidate_list_reports_omitted_names() {
        let mut graph = BuildGraph::default();
        graph
            .actions
            .insert("identity".to_owned(), command_list_action());
        for target in ["delta", "echo", "bravo", "charlie", "alpha"] {
            insert_edge(&mut graph, "identity", &[target], false);
        }
        let index = CommandListTargetIndex::from_graph(&graph);

        assert_eq!(
            index.clause(&action_identity("identity")).as_deref(),
            Some("targets \"alpha\", \"bravo\", \"charlie\" and 2 more")
        );
    }

    #[test]
    fn long_output_names_are_truncated_on_a_utf8_boundary() {
        let target = format!("{}🚀", "x".repeat(62));
        let bounded = bound_target_name(&target);

        assert!(bounded.len() <= MAX_TARGET_NAME_BYTES);
        assert!(bounded.ends_with(TRUNCATION_MARKER));
        assert!(bounded.is_char_boundary(bounded.len()));
    }

    #[test]
    fn output_names_escape_control_characters_and_quote_them() {
        let index = index_for_target("line\nbreak");

        assert_eq!(
            index.clause(&action_identity("identity")).as_deref(),
            Some("target \"line\\nbreak\"")
        );
    }

    #[test]
    fn unknown_fingerprints_have_no_clause() {
        let index = index_for_target("known");

        assert_eq!(index.clause(&"0".repeat(64)), None);
    }

    #[test]
    fn uppercase_fingerprints_resolve_case_insensitively() {
        let index = index_for_target("check-fmt");
        let fingerprint = action_identity("identity").to_ascii_uppercase();

        assert_eq!(
            index.clause(&fingerprint).as_deref(),
            Some("target \"check-fmt\"")
        );
    }

    #[test]
    fn phony_and_dependency_only_edges_are_excluded() {
        let mut graph = BuildGraph::default();
        graph
            .actions
            .insert("identity".to_owned(), command_list_action());
        graph.actions.insert(
            "dependency-only".to_owned(),
            Action {
                recipe: Recipe::Command {
                    command: StringOrList::Empty,
                    isolation: CommandIsolation::Shared,
                },
                description: None,
                depfile: None,
                deps_format: None,
                pool: None,
                restat: false,
            },
        );
        insert_edge(&mut graph, "identity", &["aggregate"], true);
        insert_edge(&mut graph, "dependency-only", &["dependency"], false);

        assert_eq!(
            CommandListTargetIndex::from_graph(&graph).clause(&action_identity("identity")),
            None
        );
    }
}
