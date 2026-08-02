//! Comparing one catalogue against the declared keys and the English source.
//!
//! The comparison is pure: it takes the parsed key sets and reports what is
//! wrong, leaving the reading of files and the walking of the registry to
//! `mod.rs`. That separation is what lets the rules below be tested directly.

use super::ftl::MessageVariables;
use std::collections::BTreeSet;

/// Findings for a single catalogue.
pub(super) struct LocaleFindings {
    tag: String,
    missing: Vec<String>,
    orphaned: Vec<String>,
    variable_mismatches: Vec<String>,
}

impl LocaleFindings {
    /// Whether the catalogue matched the declared keys and the source.
    pub(super) const fn is_clean(&self) -> bool {
        self.missing.is_empty() && self.orphaned.is_empty() && self.variable_mismatches.is_empty()
    }

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
pub(super) fn build_error_message(findings: &[LocaleFindings]) -> String {
    let mut message = String::from("localization audit failed:");
    for finding in findings {
        finding.append_to(&mut message);
    }
    message
}
