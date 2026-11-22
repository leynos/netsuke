//! Shared fixtures for command-list manifest-to-IR lowering tests.
//!
//! The manifest builders and rejection assertions live in their own module so
//! the test module stays within the size limit, and so the builders can take a
//! typed entry list instead of a bare string slice.

use super::super::*;
use crate::manifest;
use crate::recipe_shell::RecipeShell;
use test_support::fluent::normalize_fluent_isolates;

/// The hostile source path shared by the regression fixtures.
const HOSTILE_SOURCE: &str = "; New-Item pwned; Write-Output x";

/// Hold the recipe entries of one shared PowerShell command list.
#[derive(Clone, Copy, Debug)]
pub(super) struct Entries<'a>(pub(super) &'a [&'a str]);

impl<'a> From<&'a [&'a str]> for Entries<'a> {
    fn from(entries: &'a [&'a str]) -> Self {
        Self(entries)
    }
}

/// Encode one command-list entry as a YAML double-quoted scalar.
///
/// The fixture entries carry quotes, backticks, and line breaks, so the scalar
/// must be escaped rather than relying on YAML block indentation.
fn yaml_entry(entry: &str) -> String {
    let mut escaped = String::with_capacity(entry.len() + 2);
    escaped.push('"');
    for character in entry.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            other => escaped.push(other),
        }
    }
    escaped.push('"');
    escaped
}

/// Render a manifest whose recipe is one shared PowerShell command list.
fn shared_power_shell_manifest(entries: Entries<'_>, is_rule_backed: bool) -> String {
    let list = entries.0.iter().fold(String::new(), |mut list, entry| {
        list.push_str("      - ");
        list.push_str(&yaml_entry(entry));
        list.push('\n');
        list
    });
    if is_rule_backed {
        format!(
            concat!(
                "netsuke_version: '1.0.0'\n",
                "rules:\n",
                "  - name: emit\n",
                "    command:\n",
                "{list}",
                "targets:\n",
                "  - name: out.txt\n",
                "    sources: \"{source}\"\n",
                "    rule: emit\n",
            ),
            list = list,
            source = HOSTILE_SOURCE,
        )
    } else {
        format!(
            concat!(
                "netsuke_version: '1.0.0'\n",
                "targets:\n",
                "  - name: out.txt\n",
                "    sources: \"{source}\"\n",
                "    command:\n",
                "{list}",
            ),
            list = list,
            source = HOSTILE_SOURCE,
        )
    }
}

/// Lower a shared PowerShell command list into its build graph.
pub(super) fn lower_shared_power_shell_list(
    entries: Entries<'_>,
    is_rule_backed: bool,
) -> Result<BuildGraph, IrGenError> {
    let source = shared_power_shell_manifest(entries, is_rule_backed);
    let manifest = manifest::render_manifest(
        manifest::from_str(&source).expect("the fixture manifest should parse"),
        &minijinja::Environment::new(),
    )
    .expect("the fixture manifest template should render");
    BuildGraph::from_manifest_for_shell(&manifest, RecipeShell::PowerShell)
}

/// Return the interpolated entries of the graph's single command action.
pub(super) fn single_command_entries(graph: &BuildGraph) -> Vec<String> {
    let action = graph
        .actions
        .values()
        .next()
        .expect("the fixture should lower exactly one action");
    let Recipe::Command { command, .. } = &action.recipe else {
        panic!(
            "the fixture should lower a command recipe, got {:?}",
            action.recipe
        );
    };
    command.to_string_vec()
}

/// Identify the entry and region a rejection diagnostic must name.
#[derive(Clone, Copy)]
pub(super) struct RejectionCase<'a> {
    /// The entry's original template.
    pub(super) template: &'a str,
    /// The one-based index of the offending entry.
    pub(super) entry_index: usize,
    /// The language-neutral region code.
    pub(super) region: &'a str,
}

/// Assert that lowering rejected the entry that left its lexical region open.
pub(super) fn assert_unfinished_region_rejected(error: &IrGenError, case: RejectionCase<'_>) {
    let IrGenError::InvalidCommand { command, .. } = error else {
        panic!("expected an InvalidCommand rejection, got {error:?}");
    };
    assert_eq!(
        command, case.template,
        "the diagnostic must carry the unfinished entry's original template"
    );
    let rendered = normalize_fluent_isolates(&error.to_string());
    assert!(
        rendered.contains(&format!("entry {}", case.entry_index)),
        "the diagnostic must name the offending entry: {rendered}"
    );
    assert!(
        rendered.contains(case.region),
        "the diagnostic must name the unfinished {}: {rendered}",
        case.region
    );
    assert!(
        !rendered.contains("New-Item"),
        "the diagnostic must not carry interpolated path text: {rendered}"
    );
}
