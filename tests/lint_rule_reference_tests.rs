//! Contract tests binding the lint rule reference to the rule registry.
//!
//! The reference is the documentation a finding's `url` points at, so a rule
//! that ships without a section leaves every one of its diagnostics linking to
//! nothing, and a section left behind by a retired rule sends readers to advice
//! that no longer applies. These tests make both states a build failure.

use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use netsuke::lint::{RuleMeta, catalogue};
use rstest::rstest;

/// Repository-relative path of the rule reference.
const REFERENCE_PATH: &str = "docs/netsuke-linter-rules.md";

/// Read the rule reference, normalized to LF line endings.
///
/// A Windows checkout can hand back CRLF text, which would leave every
/// heading ending `\r\n` and make [`section`]'s LF-only delimiters match
/// nothing. Normalizing on read keeps the delimiters in one form rather than
/// teaching each lookup about both.
fn reference() -> Result<String> {
    Ok(test_support::fs::read_to_string(REFERENCE_PATH)
        .context("read the lint rule reference")?
        .replace("\r\n", "\n"))
}

/// Collapse every whitespace run so wrapped prose compares as written.
fn normalize(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Collect the rule names the reference documents, in document order.
fn documented_rules(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| line.strip_prefix("### "))
        .map(str::trim)
        .map(str::to_owned)
        .collect()
}

/// Borrow the section of `text` documenting `name`.
fn section<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    let heading = format!("### {name}\n");
    let start = text.find(&heading)? + heading.len();
    let rest = text.get(start..)?;
    let end = rest.find("\n### ").or_else(|| rest.find("\n## "));
    Some(end.map_or(rest, |offset| rest.get(..offset).unwrap_or(rest)))
}

/// A CRLF document must resolve once it has been through [`reference`]'s
/// normalization, and must not before it.
///
/// Both directions are asserted because only the second explains why the
/// normalization exists: without it a Windows checkout leaves every heading
/// ending `\r\n`, so `section` matches nothing and every rule looks
/// undocumented.
#[test]
fn crlf_sections_resolve_after_normalization() -> Result<()> {
    let crlf = concat!(
        "## Category\r\n",
        "\r\n",
        "### first-rule\r\n",
        "\r\n",
        "Body text.\r\n",
        "\r\n",
        "### second-rule\r\n",
        "\r\n",
        "Other body.\r\n",
    );
    ensure!(
        section(crlf, "first-rule").is_none(),
        "CRLF text should defeat the LF-only lookup, which is what normalization fixes"
    );

    let normalized = crlf.replace("\r\n", "\n");
    let body = section(&normalized, "first-rule").context("the normalized text should resolve")?;
    ensure!(
        body == "\nBody text.\n",
        "the section should stop at the next heading, got {body:?}"
    );
    Ok(())
}

/// Every registered rule is documented exactly once.
///
/// The count matters as much as the membership: a duplicated heading reads as
/// two records of one rule, and set membership alone would accept it.
#[test]
fn every_rule_is_documented_exactly_once() -> Result<()> {
    let text = reference()?;
    let headings = documented_rules(&text);
    let rows = catalogue_rows(&text);
    for meta in catalogue() {
        let heading_count = headings.iter().filter(|name| *name == meta.name).count();
        ensure!(
            heading_count == 1,
            "`{}` has {heading_count} sections; expected exactly one",
            meta.name
        );
        let anchor = format!("[`{}`](#{})", meta.name, meta.name);
        let row_count = rows
            .iter()
            .filter(|cells| cells.first().is_some_and(|cell| *cell == anchor))
            .count();
        ensure!(
            row_count == 1,
            "`{}` has {row_count} catalogue rows; expected exactly one",
            meta.name
        );
    }
    Ok(())
}

#[test]
fn the_reference_documents_every_registered_rule() -> Result<()> {
    let text = reference()?;
    let documented: BTreeSet<String> = documented_rules(&text).into_iter().collect();
    let registered: BTreeSet<String> = catalogue()
        .iter()
        .map(|meta| meta.name.to_owned())
        .collect();
    let missing: Vec<&String> = registered.difference(&documented).collect();
    let orphaned: Vec<&String> = documented.difference(&registered).collect();
    ensure!(
        missing.is_empty(),
        "the reference omits these rules: {missing:?}"
    );
    ensure!(
        orphaned.is_empty(),
        "the reference documents rules that do not exist: {orphaned:?}"
    );
    Ok(())
}

#[test]
fn the_reference_lists_every_rule_in_its_catalogue_table() -> Result<()> {
    let text = reference()?;
    let rows = catalogue_rows(&text);
    for meta in catalogue() {
        let expected = vec![
            format!("[`{}`](#{})", meta.name, meta.name),
            meta.category.as_str().to_owned(),
            meta.stage.as_str().to_owned(),
            meta.default_severity.as_str().to_owned(),
            meta.summary.to_owned(),
        ];
        ensure!(
            rows.contains(&expected),
            "the catalogue table is missing or disagrees about `{}`; expected the cells {expected:?}",
            meta.name
        );
    }
    Ok(())
}

/// Split the catalogue table into one trimmed cell list per row.
///
/// The Markdown formatter owns the padding inside a table, so the contract is
/// on the cells rather than on the rendered row: comparing raw text would make
/// this test fail whenever `make fmt` re-pads a column.
fn catalogue_rows(text: &str) -> Vec<Vec<String>> {
    text.lines()
        .filter(|line| line.starts_with("| [`"))
        .map(|line| {
            line.trim_matches('|')
                .split('|')
                .map(|cell| cell.trim().to_owned())
                .collect()
        })
        .collect()
}

/// Every rule's own section must restate its registry text verbatim.
///
/// The summary is checked in the catalogue table instead, because the section
/// opens with it as a sentence and therefore capitalizes its first word.
#[rstest]
fn each_section_states_the_registry_text(
    #[values("rationale", "remediation")] field: &str,
) -> Result<()> {
    let text = reference()?;
    for meta in catalogue() {
        let body = section(&text, meta.name)
            .with_context(|| format!("the reference should document `{}`", meta.name))?;
        let expected = normalize(field_text(meta, field));
        ensure!(
            normalize(body).contains(&expected),
            "the `{}` section does not state its {field}: {expected}",
            meta.name
        );
    }
    Ok(())
}

/// Borrow the named registry field of a rule.
fn field_text(meta: &'static RuleMeta, field: &str) -> &'static str {
    match field {
        "rationale" => meta.rationale,
        _ => meta.remediation,
    }
}

#[test]
fn every_section_shows_a_reported_and_a_fixed_manifest() -> Result<()> {
    let text = reference()?;
    for meta in catalogue() {
        let body = section(&text, meta.name)
            .with_context(|| format!("the reference should document `{}`", meta.name))?;
        ensure!(
            body.contains("Reported:") && body.contains("Fixed:"),
            "the `{}` section should show a reported and a fixed manifest",
            meta.name
        );
    }
    Ok(())
}

#[test]
fn documentation_urls_point_into_the_reference() {
    for meta in catalogue() {
        let url = meta.doc_url();
        assert!(
            url.ends_with(&format!("netsuke-linter-rules.md#{}", meta.name)),
            "`{}` links to {url}, which is not its reference section",
            meta.name
        );
    }
}
