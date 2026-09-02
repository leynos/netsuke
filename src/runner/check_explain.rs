//! The `netsuke check --explain` rule reference.
//!
//! The catalogue is generated from the rule registry, so what the command
//! prints and what the rule reference document says come from one source and
//! cannot drift.

use anyhow::Result;

use crate::cli::Cli;
use crate::lint::{RuleMeta, catalogue, registry};
use crate::localization::{self, keys};

use super::super::check_documentation::rule_documentation_url;
use super::super::error::RunnerError;
use super::super::process;
use super::json;

/// Print the reference for one rule, or for every rule when `name` is empty.
///
/// # Errors
///
/// Returns an error when `name` does not identify a registered rule, or when
/// the catalogue cannot be written.
pub(super) fn render(cli: &Cli, name: &str) -> Result<()> {
    let rules = select(name)?;
    let rendered = if cli.json {
        json::render_catalogue(&rules)?
    } else {
        render_text(&rules)
    };
    process::write_text_stdout(&rendered)
}

/// Select the rules `name` asks for.
fn select(name: &str) -> Result<Vec<&'static RuleMeta>> {
    if name.is_empty() {
        return Ok(catalogue());
    }
    let meta = registry::meta_by_name(name).ok_or_else(|| RunnerError::CheckPolicy {
        message: localization::message(keys::CHECK_RULE_UNKNOWN).with_arg("name", name.to_owned()),
    })?;
    Ok(vec![meta])
}

/// Render the catalogue as plain text.
///
/// A single rule prints its full reference; the whole catalogue prints one
/// line per rule, because a reader scanning twenty-odd rules wants the shape
/// of the set rather than every rationale.
#[must_use]
fn render_text(rules: &[&'static RuleMeta]) -> String {
    let mut rendered = String::new();
    match rules {
        [only] => write_full(&mut rendered, only),
        many => many
            .iter()
            .for_each(|meta| write_summary(&mut rendered, meta)),
    }
    rendered
}

/// Append one line to the rendered catalogue.
///
/// Writing into a `String` cannot fail, so the formatter result is discarded
/// here rather than being threaded through every caller.
fn write_line(rendered: &mut String, line: &str) {
    rendered.push_str(line);
    rendered.push('\n');
}

/// Write one rule's full reference.
fn write_full(rendered: &mut String, meta: &RuleMeta) {
    write_line(rendered, meta.name);
    write_line(
        rendered,
        &format!(
            "  category: {}    stage: {}    default: {}",
            meta.category.as_str(),
            meta.stage.as_str(),
            meta.default_severity.as_str()
        ),
    );
    write_line(rendered, &format!("  code: {}", meta.code()));
    write_line(rendered, &format!("  summary: {}", meta.summary));
    write_line(rendered, &format!("  rationale: {}", meta.rationale));
    write_line(rendered, &format!("  remediation: {}", meta.remediation));
    write_line(
        rendered,
        &format!("  documentation: {}", rule_documentation_url(meta.name)),
    );
}

/// Write one catalogue line for a rule.
fn write_summary(rendered: &mut String, meta: &RuleMeta) {
    write_line(
        rendered,
        &format!(
            "{:<30} {:<13} {:<8} {}",
            meta.name,
            meta.category.as_str(),
            meta.default_severity.as_str(),
            meta.summary
        ),
    );
}
