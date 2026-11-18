//! Cases that pin how an absolute request is matched against the recorded
//! workspace spelling.
//!
//! The match is component-wise and platform-dependent: Windows compares names
//! case-insensitively, because the same directory is commonly spelt with
//! either case, while every other platform compares them literally. Pinning
//! both halves here means the Windows branch cannot be reduced to `==` without
//! the Windows job failing, and the literal platforms cannot silently gain
//! case folding.
//!
//! The comparison is also component-wise rather than a string prefix, so a
//! sibling that merely starts with the root's letters stays outside it.

use anyhow::{Result, ensure};
use rstest::rstest;

use super::{absolute_root, resolved, root_recorded_at, scratch};

/// A request spelt in a different case from the root follows the platform's
/// name rule.
///
/// The recorded root is the lower-case `work` beneath the scratch directory
/// and the request is beneath the upper-case `WORK`, so the two differ only in
/// the case of one component. Windows resolves the request because it compares
/// names case-insensitively; every other platform refuses it, because there
/// the two spellings name different directories.
#[rstest]
fn a_case_variant_of_the_root_component_follows_the_platform_name_rule() -> Result<()> {
    let (_temp, path, _) = scratch()?;
    let recorded = absolute_root(&path);
    let root = root_recorded_at(&path, &recorded)?;
    let requested = path.join("WORK").join("file");
    let outcome = resolved(&root, requested.as_str());

    #[cfg(windows)]
    ensure!(
        outcome
            .as_deref()
            .is_some_and(|relative| relative.as_str() == "file"),
        "{requested:?} should resolve beneath {recorded:?} case-insensitively, \
         but resolved to {outcome:?}"
    );
    #[cfg(not(windows))]
    ensure!(
        outcome.is_none(),
        "{requested:?} differs from {recorded:?} in case, so it must be refused, \
         but resolved to {outcome:?}"
    );
    Ok(())
}

/// A sibling whose name only begins like the root is outside it on every
/// platform, including one that compares names case-insensitively.
///
/// `WORKSHOP` matches the root `work` up to the root's last character, so a
/// string-prefix comparison would accept it while a component-wise one does
/// not. The case difference keeps this from passing merely because the
/// platform compares literally.
#[cfg(windows)]
#[rstest]
fn a_case_insensitive_component_match_is_not_a_string_prefix_match() -> Result<()> {
    let (_temp, path, _) = scratch()?;
    let recorded = absolute_root(&path);
    let root = root_recorded_at(&path, &recorded)?;
    let requested = path.join("WORKSHOP").join("file");
    let outcome = resolved(&root, requested.as_str());
    ensure!(
        outcome.is_none(),
        "{requested:?} names a different component from {recorded:?}, so it must \
         be refused, but resolved to {outcome:?}"
    );
    Ok(())
}
