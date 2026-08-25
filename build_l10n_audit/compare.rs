//! Comparing one catalogue against the declared keys and the English source.
//!
//! The comparison is pure: it takes the parsed key sets and reports what is
//! wrong, leaving the reading of files and the walking of the registry to
//! `mod.rs`. That separation is what lets the rules below be tested directly.

use super::ftl::MessageVariables;
use std::collections::BTreeSet;

/// Findings for a single catalogue.
pub(super) struct LocaleFindings {
    /// The locale tag the findings refer to.
    tag: String,
    /// Declared keys the catalogue does not provide.
    missing: Vec<String>,
    /// Keys the catalogue provides that are not declared.
    orphaned: Vec<String>,
    /// Descriptions of keys whose interpolation variables differ from the source.
    variable_mismatches: Vec<String>,
}

impl LocaleFindings {
    /// Whether the catalogue matched the declared keys and the source.
    ///
    /// # Examples
    ///
    /// ```text
    /// missing = [], orphaned = [], variable_mismatches = []  ->  true
    /// missing = ["cli.about"], orphaned = [], mismatches = []  ->  false
    /// ```
    pub(super) const fn is_clean(&self) -> bool {
        self.missing.is_empty() && self.orphaned.is_empty() && self.variable_mismatches.is_empty()
    }

    /// Append this catalogue's findings to `message`, one line per category.
    ///
    /// Clean findings append nothing, so a passing locale leaves no trace in
    /// the build-failure text.
    ///
    /// # Examples
    ///
    /// ```text
    /// tag = "fr", missing = ["cli.about"], orphaned = ["fr.extra"]
    ///   appends: "\n- missing in fr: cli.about\n- orphaned in fr: fr.extra"
    /// tag = "fr", all empty
    ///   appends: nothing
    /// ```
    fn append_to(&self, message: &mut String) {
        append_section(message, &self.tag, "missing", &self.missing);
        append_section(message, &self.tag, "orphaned", &self.orphaned);
        append_section(
            message,
            &self.tag,
            "variable mismatch",
            &self.variable_mismatches,
        );
    }
}

/// Append one labelled finding line, or nothing when `entries` is empty.
///
/// # Examples
///
/// ```text
/// label = "missing", tag = "de", entries = ["a.key", "b.key"]
///   appends: "\n- missing in de: a.key, b.key"
/// entries = []
///   appends: nothing
/// ```
fn append_section(message: &mut String, tag: &str, label: &str, entries: &[String]) {
    if entries.is_empty() {
        return;
    }
    message.push_str("\n- ");
    message.push_str(label);
    message.push_str(" in ");
    message.push_str(tag);
    message.push_str(": ");
    message.push_str(&entries.join(", "));
}

/// Render variable names for a diagnostic, sigils included.
///
/// # Examples
///
/// ```text
/// {"count", "path"}  ->  "$count $path"
/// {}                 ->  "none"
/// ```
///
/// The empty set renders as `none` rather than an empty string so a message
/// reading "expected none, found $path" stays legible.
fn render_variables(names: &BTreeSet<String>) -> String {
    if names.is_empty() {
        return "none".to_owned();
    }
    names
        .iter()
        .map(|name| format!("${name}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Describe one key whose variables differ from the source.
///
/// # Examples
///
/// ```text
/// key = "stdlib.path.io.failed", source = {"path"}, other = {"percorso"}
///   ->  "stdlib.path.io.failed (expected $path, found $percorso)"
/// key = "cli.about", source = {}, other = {"extra"}
///   ->  "cli.about (expected none, found $extra)"
/// ```
fn describe_variable_mismatch(
    key: &str,
    source: &BTreeSet<String>,
    other: &BTreeSet<String>,
) -> String {
    format!(
        "{key} (expected {}, found {})",
        render_variables(source),
        render_variables(other)
    )
}

/// Messages whose interpolation variables differ from the English source.
///
/// Only keys present in both are compared; a key absent from the catalogue is
/// already reported as missing.
///
/// # Examples
///
/// ```text
/// source = {"a": {"path"}}, other = {"a": {"path"}}       ->  []
/// source = {"a": {"path"}}, other = {"a": {"percorso"}}   ->  ["a (expected $path, found $percorso)"]
/// source = {"a": {"path"}}, other = {}                    ->  []   (reported as missing instead)
/// ```
fn variable_mismatches(source: &MessageVariables, other: &MessageVariables) -> Vec<String> {
    source
        .iter()
        .filter_map(|(key, expected)| {
            let found = other.get(key)?;
            (found != expected).then(|| describe_variable_mismatch(key, expected, found))
        })
        .collect()
}

/// Compare one catalogue against the declared keys and the English source.
///
/// # Examples
///
/// ```text
/// declared = {"a"}, catalogue = {"a"}          ->  clean
/// declared = {"a", "b"}, catalogue = {"a"}     ->  missing  = ["b"]
/// declared = {"a"}, catalogue = {"a", "z"}     ->  orphaned = ["z"]
/// declared = {"a"}, catalogue = {"a"} with a different $variable
///                                             ->  variable_mismatches = ["a (expected …, found …)"]
/// ```
pub(super) fn audit_catalogue(
    tag: &str,
    declared: &BTreeSet<String>,
    source: &MessageVariables,
    catalogue: &MessageVariables,
) -> LocaleFindings {
    let present: BTreeSet<String> = catalogue.keys().cloned().collect();
    LocaleFindings {
        tag: tag.to_owned(),
        missing: declared.difference(&present).cloned().collect(),
        orphaned: present.difference(declared).cloned().collect(),
        variable_mismatches: variable_mismatches(source, catalogue),
    }
}

/// Render every locale's findings into one build-failure message.
///
/// Every offending locale appears, so one build reports them all rather than
/// surfacing them one rebuild at a time.
///
/// # Examples
///
/// ```text
/// [fr: missing ["a"], de: orphaned ["z"]]
///   ->  "localization audit failed:
///        - missing in fr: a
///        - orphaned in de: z"
/// []  ->  "localization audit failed:"   (callers only build this when findings exist)
/// ```
pub(super) fn build_error_message(findings: &[LocaleFindings]) -> String {
    let mut message = String::from("localization audit failed:");
    for finding in findings {
        finding.append_to(&mut message);
    }
    message
}
