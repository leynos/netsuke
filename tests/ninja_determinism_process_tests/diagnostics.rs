//! How a failed two-run comparison explains itself.
//!
//! The comparison and its *explanation* are separate jobs. The crate root
//! decides which two bundles to compare and under what perturbation; this module
//! knows what counts as the same bundle, including the one comparison whose
//! answer is "the same apart from the localized wording", and how to describe a
//! difference once one has been found. A failure message is read when the suite
//! is red, never when it is green, so keeping it beside the comparison would
//! make the comparison harder to follow without making the message any clearer.
//!
//! Split out rather than grown in place for the second reason the repository
//! applies everywhere: `CodeScene` attributed the root's
//! `String Heavy Function Arguments` finding to the *comparison's* mix of
//! arguments, and moving the explanatory helpers out is what brings the root
//! under the threshold. The label a comparison reports travels on the
//! [`Bundle`] it names rather than beside it as a third parameter, so each
//! comparison takes two bundles and no loose strings; the strings that remain
//! are read, compared, and shortened by the text helpers, whose subject is the
//! text itself.

use super::Bundle;
use anyhow::{Result, bail, ensure};

/// Assert that two runs produced the same bundle.
///
/// A sidecar is identified by its content-addressed path as well as its body:
/// two bundles differing only in a sidecar name are still different artefacts,
/// and comparing bodies alone would miss it.
pub(super) fn assert_identical(first: &Bundle, second: &Bundle) -> Result<()> {
    let fixture = compared_fixture(first, second)?;
    if first.build_file != second.build_file {
        bail!(
            "{fixture}: two processes emitted different build files\n{}",
            describe_difference(&first.build_file, &second.build_file)
        );
    }
    assert_sidecars_identical(first, second)
}

/// The fixture both bundles of a comparison came from.
///
/// Comparing bundles from two different fixtures is a caller error: every
/// message would name one fixture while describing runs over another, so the
/// guard fails loudly rather than reporting a misleading label.
fn compared_fixture(first: &Bundle, second: &Bundle) -> Result<&'static str> {
    ensure!(
        first.fixture == second.fixture,
        "a comparison needs two bundles from one fixture, got {} and {}",
        first.fixture,
        second.fixture,
    );
    Ok(first.fixture)
}

/// Assert that two runs staged the same sidecars at the same paths.
fn assert_sidecars_identical(first: &Bundle, second: &Bundle) -> Result<()> {
    let fixture = compared_fixture(first, second)?;
    if first.sidecars.len() != second.sidecars.len() {
        bail!(
            "{fixture}: two processes staged {} and {} dyndep sidecars: {:?} vs {:?}",
            first.sidecars.len(),
            second.sidecars.len(),
            sidecar_paths(first),
            sidecar_paths(second),
        );
    }
    for (left, right) in first.sidecars.iter().zip(&second.sidecars) {
        if left.path != right.path {
            bail!(
                "{fixture}: two processes staged sidecars at different paths: {} vs {}",
                left.path,
                right.path
            );
        }
        if left.content != right.content {
            bail!(
                "{fixture}: two processes staged different content for {}\n{}",
                left.path,
                describe_difference(&left.content, &right.content)
            );
        }
    }
    Ok(())
}

/// Assert that two runs differed only in their progress descriptions.
///
/// Locale is an environment parameter of the guarantee, so differing wording is
/// expected; the claim under test is that the wording is *all* that moved. A
/// line pair therefore has to be either equal or two `description` values, and
/// at least one pair has to differ — otherwise the comparison would pass while
/// proving nothing about what a locale change reaches. The sidecars carry no
/// descriptions, so they must still match exactly.
pub(super) fn assert_only_descriptions_differ(english: &Bundle, localised: &Bundle) -> Result<()> {
    let fixture = compared_fixture(english, localised)?;
    let english_lines: Vec<&str> = english.build_file.lines().collect();
    let localised_lines: Vec<&str> = localised.build_file.lines().collect();
    if english_lines.len() != localised_lines.len() {
        bail!(
            "{fixture}: a locale change altered the build file's line count, {} vs {}",
            english_lines.len(),
            localised_lines.len(),
        );
    }

    let mut differences = 0_usize;
    for (index, (left, right)) in english_lines.iter().zip(&localised_lines).enumerate() {
        if left == right {
            continue;
        }
        ensure!(
            is_description_line(left) && is_description_line(right),
            "{fixture}: a locale change reached more than the progress descriptions, \
             at line {}:\n  english:   {}\n  localised: {}",
            index + 1,
            truncate(left),
            truncate(right),
        );
        differences += 1;
    }
    ensure!(
        differences > 0,
        "{fixture}: the two locales emitted identical descriptions, so this run \
         cannot show that a locale change reaches anything at all",
    );
    assert_sidecars_identical(english, localised)
}

/// Whether a build-file line assigns an edge's `description` value.
fn is_description_line(line: &str) -> bool {
    line.trim_start().starts_with("description = ")
}

/// The sidecar paths of one bundle, for a diagnostic.
fn sidecar_paths(bundle: &Bundle) -> Vec<&str> {
    bundle
        .sidecars
        .iter()
        .map(|sidecar| sidecar.path.as_str())
        .collect()
}

/// Describe the first line on which two texts differ.
///
/// Lines are truncated because an emitted command can run to hundreds of
/// characters, and the reader needs the location and a recognizable fragment
/// rather than the whole wrapper.
fn describe_difference(first: &str, second: &str) -> String {
    for (index, (left, right)) in first.lines().zip(second.lines()).enumerate() {
        if left != right {
            return format!(
                "first difference at line {}:\n  first:  {}\n  second: {}",
                index + 1,
                truncate(left),
                truncate(right),
            );
        }
    }
    format!(
        "one text is a prefix of the other: {} and {} lines",
        first.lines().count(),
        second.lines().count(),
    )
}

/// Shorten one line to a diagnostic-friendly length.
///
/// The prefix is rebuilt character by character rather than sliced at a byte
/// offset: the line may contain non-ASCII text, and a byte slice would panic
/// if its end fell inside a multi-byte character.
fn truncate(line: &str) -> String {
    const LIMIT: usize = 200;
    if line.len() <= LIMIT {
        return line.to_owned();
    }
    let mut prefix = String::with_capacity(LIMIT + '…'.len_utf8());
    for character in line.chars() {
        if prefix.len() + character.len_utf8() > LIMIT {
            break;
        }
        prefix.push(character);
    }
    prefix.push('…');
    prefix
}
