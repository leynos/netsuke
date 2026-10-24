//! How a failed two-run comparison explains itself.
//!
//! The comparison and its *explanation* are separate jobs. The crate root knows
//! what counts as the same bundle; this module knows how to describe a
//! difference once one has been found. A failure message is read when the suite
//! is red, never when it is green, so keeping it beside the comparison would
//! make the comparison harder to follow without making the message any clearer.
//!
//! Split out rather than grown in place for the second reason the repository
//! applies everywhere: `CodeScene` attributed the root's
//! `String Heavy Function Arguments` finding to the *comparison's* mix of
//! arguments, and moving the explanatory helpers out is what brings both files
//! under the threshold without changing a single assertion.

use super::Bundle;
use anyhow::{Result, bail};

/// Assert that two runs produced the same bundle.
///
/// A sidecar is identified by its content-addressed path as well as its body:
/// two bundles differing only in a sidecar name are still different artefacts,
/// and comparing bodies alone would miss it.
pub(super) fn assert_identical(fixture: &str, first: &Bundle, second: &Bundle) -> Result<()> {
    if first.build_file != second.build_file {
        bail!(
            "{fixture}: two processes emitted different build files\n{}",
            describe_difference(&first.build_file, &second.build_file)
        );
    }
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
