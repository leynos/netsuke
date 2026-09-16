//! Contract tests for the `Status:` header field of every `ExecPlan`.
//!
//! `ExecPlan`s under `docs/execplans/` each open with a single `Status:` line.
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
//! field in the same header fails.
//!
//! The authority order the tests cannot check is recorded in the style guide:
//! a plan's `Progress` and `Outcomes & retrospective` sections, and its
//! roadmap checkbox, outrank the header. This suite keeps the header
//! well-formed, not correct in substance.

use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};

/// The prefix marking an `ExecPlan`'s header status field.
const STATUS_PREFIX: &str = "Status:";

/// The closed set of values the header status field may carry.
///
/// Mirrors the vocabulary table in the `### ExecPlan` section of
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

/// The file names of every `ExecPlan`, sorted.
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
        fields.len() == 1,
        "the ExecPlan header should carry exactly one `{STATUS_PREFIX}` line above the \
         first section; found {}: {fields:?}",
        fields.len()
    );

    // One field, so the strip cannot fail; the fallback keeps the function
    // panic-free rather than trusting that invariant at a distance.
    let value = fields
        .first()
        .and_then(|field| field.strip_prefix(STATUS_PREFIX))
        .unwrap_or_default()
        .trim();
    Ok(value.to_owned())
}

/// Verify every `ExecPlan` header carries exactly one status from the closed set.
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
    let after_heading = source
        .split("### ExecPlan")
        .nth(1)
        .context("the style guide should carry an `### ExecPlan` section")?;
    let section = after_heading.split("\n## ").next().unwrap_or(after_heading);

    let documented: BTreeSet<&str> = table_rows(section).collect();
    let missing: BTreeSet<&str> = STATUS_VALUES
        .iter()
        .copied()
        .filter(|value| !documented.contains(value))
        .collect();
    ensure!(
        missing.is_empty(),
        "the style guide's ExecPlan section should document every accepted status; \
         missing: {missing:?}"
    );
    Ok(())
}

/// The first column of every Markdown table row in a section.
///
/// Reading the table's first column rather than scanning the section as text
/// keeps the check honest: the section names the rejected values too, in the
/// bullet giving `Status: COMPLETED` as a counter-example, so a substring
/// search would find `COMPLETE` inside `COMPLETED` and pass with the row
/// deleted.
fn table_rows(section: &str) -> impl Iterator<Item = &str> {
    section.lines().filter_map(|raw| {
        let rest = raw.trim().strip_prefix('|')?;
        let (first_cell, _) = rest.split_once('|')?;
        let cell = first_cell.trim();
        // The delimiter row separates header from body; its cells are dashes.
        (!cell.is_empty() && !cell.starts_with('-')).then_some(cell)
    })
}

#[cfg(test)]
mod tests {
    //! Unit tests for the header parsing and table reading above.
    //!
    //! Each covers one rule the contract tests depend on, so a change to
    //! `header_lines`, `header_status`, or `table_rows` fails here with a
    //! narrow cause rather than as an unexplained contract failure.

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

    #[test]
    fn table_rows_yield_the_first_cell_of_body_rows_only() -> Result<()> {
        let section = "Some prose.\n\n\
             | Value  | Meaning |\n\
             | ------ | ------- |\n\
             | DRAFT  | Draft.  |\n\
             | BLOCKED | Blocked. |\n\n\
             Trailing prose naming COMPLETED.\n";
        let rows: Vec<&str> = table_rows(section).collect();
        ensure!(
            rows == ["Value", "DRAFT", "BLOCKED"],
            "table rows should carry the header and body first cells, found {rows:?}"
        );
        ensure!(
            !rows.contains(&"COMPLETE"),
            "prose outside the table must not contribute rows, found {rows:?}"
        );
        Ok(())
    }

    #[test]
    fn table_rows_ignore_a_row_whose_first_cell_is_empty() -> Result<()> {
        let section = "| | Meaning |\n| - | ------- |\n";
        let rows: Vec<&str> = table_rows(section).collect();
        ensure!(
            rows.is_empty(),
            "a row with a blank first cell should contribute nothing, found {rows:?}"
        );
        Ok(())
    }
}
