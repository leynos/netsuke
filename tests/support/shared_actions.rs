//! Shared helpers for parsing `leynos/shared-actions` workflow references.
//!
//! Two integration tests consume these with deliberately different scopes:
//! `workflow_shared_actions_pins` sweeps every workflow and asserts only the
//! *shape* of each reference (a full lowercase commit SHA rather than a
//! branch or tag), while `polonius_toolchain_contract` additionally requires
//! the references across its checked workflows to *agree*, deriving the
//! expected pin instead of restating it so a partial bump fails while a
//! complete bump needs no test edit.

use anyhow::{Context, Result, ensure};
use std::collections::BTreeSet;

/// Path prefix common to every shared-actions action reference.
pub const SHARED_ACTIONS_MARKER: &str = "leynos/shared-actions/.github/actions/";

/// Extracts every shared-actions `uses:` token from workflow `contents`.
///
/// Each returned token is the full reference, for example
/// `leynos/shared-actions/.github/actions/setup-rust@<sha>`.
#[must_use]
pub fn extract_shared_actions_uses(contents: &str) -> Vec<String> {
    contents
        .lines()
        .filter_map(|line| {
            let start = line.find(SHARED_ACTIONS_MARKER)?;
            let rest = line.get(start..)?;
            let token = rest.split_whitespace().next()?;
            (!token.is_empty()).then(|| token.to_owned())
        })
        .collect()
}

/// Splits a shared-actions reference into its action name and pin.
///
/// Returns `None` when the reference lacks the marker prefix or an `@`.
#[must_use]
pub fn split_shared_action_ref(reference: &str) -> Option<(&str, &str)> {
    reference
        .strip_prefix(SHARED_ACTIONS_MARKER)?
        .split_once('@')
}

/// Reports whether `pin` is a full 40-character lowercase-hex commit SHA.
#[must_use]
pub fn is_commit_sha_pin(pin: &str) -> bool {
    pin.len() == 40
        && pin
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

/// Returns the single commit SHA shared by every reference in `refs`.
///
/// # Errors
///
/// Fails when `refs` is empty, when any reference is malformed or pinned to
/// something other than a commit SHA, or when the references disagree.
pub fn consistent_pin(refs: &[String]) -> Result<String> {
    ensure!(
        !refs.is_empty(),
        "expected at least one shared-actions reference to derive the pin from"
    );
    let mut shas = BTreeSet::new();
    for reference in refs {
        let (name, pin) = split_shared_action_ref(reference)
            .with_context(|| format!("malformed shared-actions reference {reference:?}"))?;
        ensure!(
            !name.is_empty() && is_commit_sha_pin(pin),
            "shared-actions reference should pin a 40-hex commit SHA, found {reference:?}"
        );
        shas.insert(pin.to_owned());
    }
    ensure!(
        shas.len() == 1,
        "shared-actions references disagree on the pin: {shas:?}"
    );
    shas.into_iter()
        .next()
        .context("one shared-actions pin should remain")
}
