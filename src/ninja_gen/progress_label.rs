//! Localized, bounded descriptions for inline-recipe Ninja edges.
//!
//! Ninja generation owns this formatting policy. It uses only edge-local
//! inline provenance and the resolved action recipe shape; target discovery
//! descriptions and command text belong to separate interfaces. Callers must
//! resolve the localizer once for a generation operation and pass it here.

use crate::ast::{Recipe, StringOrList};
use crate::ir::{Action, BuildEdge};
use crate::localization::{self, keys};
use crate::terminal_text::{bound_scalars, terminal_safe};
use ortho_config::Localizer;

/// Keep localized edge descriptions readable in Ninja progress output.
const PROGRESS_NAME_LIMIT: usize = 80;

/// Render an edge's inline-recipe progress label when it has executable work.
///
/// Sanitize and bound the edge-local name before interpolation. The recipe is
/// inspected only to distinguish scalar/script recipes from command lists and
/// to count list entries.
///
/// # Examples
///
/// With the English catalogue, an inline recipe for `check-fmt` renders as
/// `Building check-fmt`.
#[must_use]
pub(crate) fn for_edge(
    edge: &BuildEdge,
    action: &Action,
    localizer: &dyn Localizer,
) -> Option<String> {
    let name = edge
        .progress_label
        .as_deref()
        .filter(|name| !name.is_empty())?;
    if action.recipe.is_dependency_only() {
        return None;
    }

    let safe_name = terminal_safe(name);
    let bounded_name = bound_scalars(&safe_name, PROGRESS_NAME_LIMIT);
    let scalar_label = || {
        let rendered_message = localization::message(keys::NINJA_GEN_PROGRESS_INLINE)
            .with_arg("name", bounded_name.as_ref())
            .render_with(localizer);
        remove_fluent_isolates(&rendered_message)
    };

    match &action.recipe {
        Recipe::Command {
            command: StringOrList::List(commands),
            ..
        } => {
            let rendered_message = localization::message(keys::NINJA_GEN_PROGRESS_COMMAND_LIST)
                .with_arg("name", bounded_name.as_ref())
                .with_count("count", commands.len())
                .render_with(localizer);
            Some(remove_fluent_isolates(&rendered_message))
        }
        Recipe::Command {
            command: StringOrList::String(_),
            ..
        }
        | Recipe::Script { .. } => Some(scalar_label()),
        Recipe::Command {
            command: StringOrList::Empty,
            ..
        }
        | Recipe::Rule { .. } => None,
    }
}

/// Remove the invisible directional wrappers Fluent adds around arguments.
///
/// The raw name is sanitized before localization, so these remaining
/// `FSI`/`PDI` characters are formatter scaffolding rather than manifest text.
/// For example, Fluent's `Building \u{2068}check-fmt\u{2069}` becomes
/// `Building check-fmt`.
fn remove_fluent_isolates(message: &str) -> String {
    message
        .chars()
        .filter(|character| !matches!(character, '\u{2068}' | '\u{2069}'))
        .collect()
}

#[cfg(test)]
mod tests {
    //! Pin label localization, sanitization and Ninja metadata escaping.

    use super::for_edge;
    use crate::ast::{CommandIsolation, Recipe, StringOrList};
    use crate::cli_localization::build_localizer;
    use crate::ir::{Action, BuildEdge, BuildGraph, DependencyOrder};
    use crate::ninja_gen::escape::escape_metadata_value;
    use crate::ninja_gen::{RecipeShell, generate_with_shell};
    use camino::Utf8PathBuf;
    use proptest::prelude::*;
    use rstest::rstest;
    use std::fmt::Write as _;

    /// Build a scalar command action for a progress-label case.
    ///
    /// For example, `scalar_action()` uses `echo ready` as its recipe.
    fn scalar_action() -> Action {
        Action {
            recipe: Recipe::Command {
                command: StringOrList::String("echo ready".into()),
                isolation: CommandIsolation::Shared,
            },
            description: None,
            depfile: None,
            deps_format: None,
            pool: None,
            restat: false,
        }
    }

    /// Build an edge whose explicit output provides its generated target.
    ///
    /// For example, `edge(Some("check-fmt"))` retains that raw label.
    fn edge(progress_label: Option<&str>) -> BuildEdge {
        BuildEdge {
            action_id: "progress".into(),
            inputs: Vec::new(),
            implicit_deps: Vec::new(),
            dependency_order: DependencyOrder::Parallel,
            explicit_outputs: vec![Utf8PathBuf::from("out")],
            implicit_outputs: Vec::new(),
            order_only_deps: Vec::new(),
            progress_label: progress_label.map(str::to_owned),
            phony: false,
            always: false,
        }
    }

    /// Escape localized labels through the same metadata path as edge output.
    #[rstest]
    #[case::dollar("cash$target", "Building cash$$target")]
    #[case::newline("left\nright", "Building left\\nright")]
    #[case::bidi("left\u{202e}right", "Building left\\u{202e}right")]
    fn scalar_labels_are_sanitized_and_ninja_escaped(#[case] name: &str, #[case] expected: &str) {
        let action = scalar_action();
        let edge = edge(Some(name));
        let localizer = build_localizer(Some("en-US"));
        let localized = for_edge(&edge, &action, localizer.as_ref())
            .expect("the inline scalar edge has a progress label");
        let escaped = escape_metadata_value(Some(&localized))
            .expect("the localized description is valid Ninja metadata");

        assert_eq!(
            escaped.map(|value| value.to_string()).as_deref(),
            Some(expected)
        );
    }

    /// Bound progress names to 80 Unicode scalar values without splitting UTF-8.
    #[rstest]
    #[case::short(12, false)]
    #[case::at_limit(80, false)]
    #[case::over_limit(81, true)]
    fn scalar_labels_obey_the_unicode_bound(#[case] length: usize, #[case] truncated: bool) {
        let name = ["é", "界", "🦀"]
            .into_iter()
            .cycle()
            .take(length)
            .collect::<Vec<_>>()
            .concat();
        let action = scalar_action();
        let edge = edge(Some(&name));
        let localizer = build_localizer(Some("en-US"));
        let localized = for_edge(&edge, &action, localizer.as_ref())
            .expect("the inline scalar edge has a progress label");
        let bounded_name = localized
            .strip_prefix("Building ")
            .expect("the English catalogue prefixes scalar labels");

        assert_eq!(bounded_name.chars().count(), length.min(80));
        assert_eq!(bounded_name.ends_with('…'), truncated);
    }

    /// Select singular and plural Fluent variants with the numeric list count.
    #[rstest]
    #[case::singular(1, "Building check-fmt (1 command)")]
    #[case::plural(2, "Building check-fmt (2 commands)")]
    fn command_list_labels_include_localized_counts(#[case] count: usize, #[case] expected: &str) {
        let action = Action {
            recipe: Recipe::Command {
                command: StringOrList::List(vec!["echo ready".into(); count]),
                isolation: CommandIsolation::Shared,
            },
            ..scalar_action()
        };
        let edge = edge(Some("check-fmt"));
        let localizer = build_localizer(Some("en-US"));

        assert_eq!(
            for_edge(&edge, &action, localizer.as_ref()).as_deref(),
            Some(expected)
        );
    }

    proptest! {
        #[test]
        fn command_list_labels_match_generated_english_contract(
            name in "[a-zA-Z0-9_-]{1,120}",
            count in 1_usize..=64,
        ) {
            let expected_name = if name.len() > 80 {
                format!("{}…", name.chars().take(79).collect::<String>())
            } else {
                name.clone()
            };
            let expected_plural = if count == 1 { "command" } else { "commands" };
            let expected = format!("Building {expected_name} ({count} {expected_plural})");
            let action = Action {
                recipe: Recipe::Command {
                    command: StringOrList::List(vec!["echo ready".into(); count]),
                    isolation: CommandIsolation::Shared,
                },
                ..scalar_action()
            };
            let edge = edge(Some(&name));
            let localizer = build_localizer(Some("en-US"));
            let actual = for_edge(&edge, &action, localizer.as_ref());

            prop_assert_eq!(actual.as_deref(), Some(expected.as_str()));
        }

        #[test]
        fn inline_labels_sanitize_arbitrary_unicode_and_controls(
            unicode_scalars in prop::collection::vec(any::<char>(), 0..=120),
            injected_controls in prop::collection::vec(
                prop_oneof![
                    (0_u8..=0x1f).prop_map(char::from),
                    (0x7f_u8..=0x9f).prop_map(char::from),
                    prop::sample::select(vec![
                        '\u{061c}', '\u{200e}', '\u{200f}', '\u{202a}',
                        '\u{202b}', '\u{202c}', '\u{202d}', '\u{202e}',
                        '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}',
                    ]),
                ],
                1..=4,
            ),
        ) {
            let mut raw_name: String = injected_controls.iter().copied().collect();
            raw_name.extend(unicode_scalars.iter().copied());
            let is_terminal_control = |character: char| matches!(
                character,
                '\0'..='\u{001f}'
                    | '\u{007f}'..='\u{009f}'
                    | '\u{061c}' | '\u{200e}' | '\u{200f}'
                    | '\u{202a}'..='\u{202e}'
                    | '\u{2066}'..='\u{2069}'
            );
            let mut expected_safe_name = String::new();
            for character in raw_name.chars() {
                match character {
                    '\n' => expected_safe_name.push_str("\\n"),
                    '\r' => expected_safe_name.push_str("\\r"),
                    '\t' => expected_safe_name.push_str("\\t"),
                    control if is_terminal_control(control) => {
                        write!(expected_safe_name, "\\u{{{:x}}}", u32::from(control))
                            .expect("writing to a string cannot fail");
                    }
                    printable => expected_safe_name.push(printable),
                }
            }
            let expected_name = if expected_safe_name.chars().count() > 80 {
                let mut prefix: String = expected_safe_name
                    .chars()
                    .take(79)
                    .collect();
                prefix.push('…');
                prefix
            } else {
                expected_safe_name
            };
            let action = scalar_action();
            let edge = edge(Some(&raw_name));
            let localizer = build_localizer(Some("en-US"));
            let actual = for_edge(&edge, &action, localizer.as_ref())
                .expect("the inline scalar edge has a progress label");
            let expected = format!("Building {expected_name}");

            prop_assert!(expected_name.chars().count() <= 80);
            prop_assert_eq!(actual.as_str(), expected.as_str());
            prop_assert!(actual.chars().all(|character| !is_terminal_control(character)));
        }
    }

    /// Leave directly constructed edges without provenance undescribed.
    #[test]
    fn direct_ir_without_progress_label_emits_no_edge_description() {
        let mut graph = BuildGraph::default();
        graph.actions.insert("progress".into(), scalar_action());
        graph
            .insert_edge(edge(None))
            .expect("the direct IR output alias is unique");

        let ninja = generate_with_shell(&graph, RecipeShell::Posix)
            .expect("the direct IR recipe should generate");

        assert!(
            !ninja.contains("description ="),
            "no edge description should be synthesized without IR provenance: {ninja}"
        );
    }
}
