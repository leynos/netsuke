//! Checking derived section references against RFC 0006 section 8.
//!
//! Every accept row cites the section 8 subsection that specifies the helper it
//! accepts. This module is what makes that citation load-bearing: a row citing
//! `§8.9` for a helper section 8.9 never mentions is a row whose contract does
//! not exist, and the coverage map would hand that helper to a child RFC with
//! nothing to implement against.
//!
//! Heading anchoring is used here, and only here. Section 8's subsections *are*
//! headings, so looking one up by number is exact. The survey's accepted set is
//! never derived this way; see the parent module for why.

use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};

use super::{Row, Section, heading_depth};

/// Assert every accepted helper is named in the section 8 subsection it cites.
pub(super) fn check_section_8(
    document: &Section<'_>,
    accepted: &BTreeMap<String, Row>,
    sections_of: &BTreeMap<String, String>,
) -> Result<()> {
    let section_8 = document
        .subsection("## 8. Accepted capabilities")
        .context("RFC 0006 has no section 8")?;
    let mut unsectioned = Vec::new();
    let mut unmentioned = Vec::new();
    for name in accepted.keys() {
        let Some(section) = sections_of.get(name) else {
            unsectioned.push(name.clone());
            continue;
        };
        let Some(lines) = subsection_lines(&section_8, section) else {
            unsectioned.push(name.clone());
            continue;
        };
        if !mentions(&lines, name) {
            unmentioned.push(format!("{name} (cited section {section})"));
        }
    }
    ensure!(
        unsectioned.is_empty(),
        "accepted helpers {unsectioned:?} name no RFC 0006 section 8 subsection, so they have no \
         contract to implement"
    );
    ensure!(
        unmentioned.is_empty(),
        "accepted helpers {unmentioned:?} are never named as whole tokens in the section 8 \
         subsection that specifies them"
    );
    Ok(())
}

/// The lines of the section 8 subsection headed `### N.N. …`.
fn subsection_lines<'a>(section_8: &Section<'a>, number: &str) -> Option<Vec<&'a str>> {
    let prefix = format!("### {number}.");
    let start = section_8
        .lines
        .iter()
        .position(|line| line.trim().starts_with(&prefix))?;
    let depth = heading_depth(section_8.lines.get(start)?)?;
    let rest = section_8.lines.get(start + 1..)?;
    let end = rest
        .iter()
        .position(|line| heading_depth(line).is_some_and(|d| d <= depth))
        .unwrap_or(rest.len());
    rest.get(..end).map(<[&str]>::to_vec)
}

/// Whether `name` appears in `lines` as a whole identifier token.
///
/// Splitting on identifier characters is what keeps `difference` from matching
/// inside `symmetric_difference`, `abs` inside `is_abs`, `quote` inside
/// `shell_quote`, and `hash` inside `text_hash`.
fn mentions(lines: &[&str], name: &str) -> bool {
    lines.iter().any(|line| {
        line.split(|ch: char| !(ch.is_alphanumeric() || ch == '_'))
            .any(|token| token == name)
    })
}
