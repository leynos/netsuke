//! Contract tests for the `Status:` header field of every ExecPlan.
//!
//! ExecPlans under `docs/execplans/` each open with a single `Status:` line.
//! The documentation style guide defines its value as a closed set, and the
//! field's whole value is that it can be grepped, counted, and checked without
//! reading the plan. Issue #586 found that it had instead drifted: five
//! vocabularies for the same state, three plans with no field at all, and two
//! that disagreed with their own `Progress` sections.
//!
//! These tests are the mechanical check that keeps the field honest. They
//! assert the *shape* of the field, never a particular plan's status, so a
//! plan moving from `IN PROGRESS` to `COMPLETE` needs no test change — only a
//! stray value, a missing line, a qualifier glued to the value, or a second
//! field further down fails.
//!
//! The authority order the tests cannot check is recorded in the style guide:
//! a plan's `Progress` and `Outcomes & retrospective` sections, and its
//! roadmap checkbox, outrank the header. This suite keeps the header
//! well-formed, not correct in substance.

use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};

/// The prefix marking an ExecPlan's header status field.
const STATUS_PREFIX: &str = "Status:";

/// The closed set of values the header status field may carry.
///
/// Mirrors the table under *ExecPlan* in
/// `docs/documentation-style-guide.md`, which this suite also checks for
/// agreement so the two cannot drift apart.
const STATUS_VALUES: [&str; 5] = ["DRAFT", "APPROVED", "IN PROGRESS", "BLOCKED", "COMPLETE"];

/// The style-guide section that defines the status vocabulary.
const STYLE_GUIDE: &str = "docs/documentation-style-guide.md";

/// Opens the repository root as a capability-scoped directory handle.
///
/// Every read goes through this handle, so the suite cannot reach outside the
/// checkout.
///
/// # Errors
///
/// Returns an error when the manifest directory cannot be opened.
fn repo_root() -> Result<Dir> {
    let root = Utf8Path::new(env!("CARGO_MANIFEST_DIR"));
    Dir::open_ambient_dir(root, ambient_authority())
        .with_context(|| format!("open repository root {root}"))
}

/// The file names of every ExecPlan, sorted.
///
/// # Errors
///
/// Returns an error when the directory cannot be read.
fn execplan_names(root: &Dir) -> Result<Vec<String>> {
    let mut names = Vec::new();
    for entry_result in root
        .read_dir("docs/execplans")
        .context("read the execplans directory")?
    {
        let directory_entry = entry_result.context("read an execplans directory entry")?;
        let name = directory_entry
            .file_name()
            .context("read an execplans entry name")?;
        if Utf8Path::new(&name).extension() == Some("md") {
            names.push(name);
        }
    }
    names.sort();
    ensure!(
        !names.is_empty(),
        "docs/execplans should contain at least one ExecPlan"
    );
    Ok(names)
}

/// The lines of a plan that precede its first section heading.
///
/// The header is everything above the first `## ` heading, which is where the
/// style guide puts the status field.
fn header_lines(source: &str) -> Vec<&str> {
    source
        .lines()
        .take_while(|line| !line.starts_with("## "))
        .collect()
}

/// The value of a plan's header status field.
///
/// # Errors
///
/// Returns an error when the header carries no status field at all, or carries
/// more than one.
fn header_status(source: &str) -> Result<String> {
    let lines = header_lines(source);
    let fields: Vec<&str> = lines
        .iter()
        .copied()
        .filter(|line| line.starts_with(STATUS_PREFIX))
        .collect();

    ensure!(
        !fields.is_empty(),
        "the ExecPlan header should carry a `{STATUS_PREFIX}` line above the first section"
    );
    ensure!(
        fields.len() == 1,
        "the ExecPlan header should carry exactly one `{STATUS_PREFIX}` line; found {}: {fields:?}",
        fields.len()
    );

    let value = fields[0]
        .strip_prefix(STATUS_PREFIX)
        .unwrap_or_default()
        .trim();
    Ok(value.to_owned())
}

/// Verify every ExecPlan header declares exactly one status from the closed set.
///
/// The value is reported for a plan that breaks the rule, so the failure names
/// the offending file and the stray value in one line.
#[test]
fn every_execplan_header_status_is_within_the_closed_set() -> Result<()> {
    let root = repo_root()?;
    let mut offenders = Vec::new();

    for name in execplan_names(&root)? {
        let path = Utf8Path::new("docs/execplans").join(&name);
        let source = root
            .read_to_string(&path)
            .with_context(|| format!("read {path}"))?;
        let value = header_status(&source)
            .with_context(|| format!("{path} should declare a well-formed status"))?;
        if !STATUS_VALUES.contains(&value.as_str()) {
            offenders.push(format!("{name}: {value:?}"));
        }
    }

    ensure!(
        offenders.is_empty(),
        "every ExecPlan status must be one of {STATUS_VALUES:?}; \
         off-vocabulary or qualified values found: {}",
        offenders.join(", ")
    );
    Ok(())
}

/// Verify the style guide still defines the vocabulary the tests enforce.
///
/// Without this, the suite could keep passing against a set the guide no
/// longer states — the tests would be checking a rule nobody publishes.
#[test]
fn the_style_guide_defines_every_accepted_status_value() -> Result<()> {
    let root = repo_root()?;
    let source = root
        .read_to_string(STYLE_GUIDE)
        .context("read the documentation style guide")?;
    let section = source
        .split("### ExecPlan")
        .nth(1)
        .context("the style guide should carry an `### ExecPlan` section")?;
    let section = section.split("\n## ").next().unwrap_or(section);

    let missing: BTreeSet<&str> = STATUS_VALUES
        .iter()
        .copied()
        .filter(|value| !section.contains(value))
        .collect();
    ensure!(
        missing.is_empty(),
        "the style guide's ExecPlan section should name every accepted status; \
         missing: {missing:?}"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_lines_stop_at_the_first_section() -> Result<()> {
        let source = "# Title\n\nStatus: COMPLETE\n\n## Purpose\n\nStatus: STALE\n";
        let lines = header_lines(source);
        ensure!(
            !lines.iter().any(|line| line.contains("STALE")),
            "the header should end at the first `## ` heading, found {lines:?}"
        );
        ensure!(
            lines.iter().any(|line| line.contains("COMPLETE")),
            "the header should keep lines above the first heading, found {lines:?}"
        );
        Ok(())
    }

    #[test]
    fn header_status_takes_the_bare_value() -> Result<()> {
        let source = "# Title\n\nStatus: IN PROGRESS\n\n## Purpose\n";
        ensure!(
            header_status(source)? == "IN PROGRESS",
            "the status should be read without its prefix or surrounding space"
        );
        Ok(())
    }

    #[test]
    fn header_status_rejects_a_missing_field() -> Result<()> {
        let source = "# Title\n\n## Purpose\n";
        let outcome = header_status(source);
        ensure!(
            outcome.is_err(),
            "a plan with no status field should be rejected, found {outcome:?}"
        );
        Ok(())
    }

    #[test]
    fn header_status_rejects_a_duplicated_field() -> Result<()> {
        let source = "# Title\n\nStatus: COMPLETE\nStatus: IN PROGRESS\n\n## Purpose\n";
        let outcome = header_status(source);
        ensure!(
            outcome.is_err(),
            "a plan carrying two status fields should be rejected, found {outcome:?}"
        );
        Ok(())
    }

    #[test]
    fn a_qualified_value_is_outside_the_closed_set() -> Result<()> {
        let source = "# Title\n\nStatus: COMPLETE (Stage D completed)\n\n## Purpose\n";
        let value = header_status(source)?;
        ensure!(
            !STATUS_VALUES.contains(&value.as_str()),
            "a qualified status should not satisfy the closed set, found {value:?}"
        );
        Ok(())
    }
}
