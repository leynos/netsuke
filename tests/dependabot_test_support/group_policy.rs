//! Dependabot group policy: one minor-and-patch catch-all per ecosystem.
//!
//! Routine bumps arrive as one grouped pull request per ecosystem, while each
//! major update arrives on its own. A narrow lockstep group, such as the
//! `RustCrypto` family, is the only permitted exception, and it must precede the
//! catch-all because Dependabot assigns a dependency to the first group whose
//! patterns match it.

use anyhow::{Result, ensure};
use indexmap::IndexMap;
use rstest::rstest;
use serde::Deserialize;

/// Parsed Dependabot group definition.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct DependabotGroup {
    #[serde(default)]
    patterns: Vec<String>,
    #[serde(default)]
    update_types: Vec<String>,
}

/// Return whether a group matches every dependency but only minor and patch updates.
fn is_minor_and_patch_catch_all(group: &DependabotGroup) -> bool {
    let mut update_types = group
        .update_types
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    update_types.sort_unstable();
    group.patterns == ["*"] && update_types == ["minor", "patch"]
}

/// Assert that an ecosystem has one trailing minor-and-patch catch-all group and
/// no other group beyond the named lockstep exceptions.
///
/// # Errors
///
/// Returns an error naming the ecosystem when the catch-all is missing,
/// duplicated, or not listed last, or when any other group would take majors.
pub(crate) fn assert_group_policy(
    ecosystem: &str,
    groups: &IndexMap<String, DependabotGroup>,
    lockstep_exceptions: &[&str],
) -> Result<()> {
    let catch_alls = groups
        .iter()
        .filter(|(_, group)| is_minor_and_patch_catch_all(group))
        .map(|(name, _)| name.as_str())
        .collect::<Vec<_>>();
    ensure!(
        catch_alls.len() == 1,
        "{ecosystem} should define exactly one minor-and-patch catch-all group, found {catch_alls:?}"
    );
    let unexpected = groups
        .keys()
        .map(String::as_str)
        .filter(|name| !catch_alls.contains(name) && !lockstep_exceptions.contains(name))
        .collect::<Vec<_>>();
    ensure!(
        unexpected.is_empty(),
        "{ecosystem} groups {unexpected:?} would group major updates; majors must arrive ungrouped"
    );
    ensure!(
        groups.keys().last().map(String::as_str) == catch_alls.first().copied(),
        "{ecosystem} catch-all group must be listed last so lockstep groups claim their members first"
    );
    Ok(())
}

/// Parse a YAML `groups` mapping fixture.
fn groups_from(yaml: &str) -> Result<IndexMap<String, DependabotGroup>> {
    Ok(serde_yaml::from_str(yaml)?)
}

/// A lone catch-all, and a lockstep exception listed before it, both conform.
#[rstest]
#[case::catch_all_only("all:\n  patterns: ['*']\n  update-types: [minor, patch]\n", &[])]
#[case::lockstep_first(
    "rustcrypto:\n  patterns: [sha2]\nall:\n  patterns: ['*']\n  update-types: [patch, minor]\n",
    &["rustcrypto"]
)]
fn conforming_groups_pass(#[case] yaml: &str, #[case] exceptions: &[&str]) -> Result<()> {
    assert_group_policy("cargo", &groups_from(yaml)?, exceptions)
}

/// Each way of letting majors into a group, or of losing the catch-all, is refused.
#[rstest]
#[case::no_groups("{}\n", &[])]
#[case::catch_all_takes_majors("all:\n  patterns: ['*']\n", &[])]
#[case::catch_all_names_major("all:\n  patterns: ['*']\n  update-types: [major, minor, patch]\n", &[])]
#[case::two_catch_alls(
    "a:\n  patterns: ['*']\n  update-types: [minor, patch]\nb:\n  patterns: ['*']\n  update-types: [minor, patch]\n",
    &[]
)]
#[case::unlisted_narrow_group(
    "rustcrypto:\n  patterns: [sha2]\nall:\n  patterns: ['*']\n  update-types: [minor, patch]\n",
    &[]
)]
#[case::catch_all_before_lockstep(
    "all:\n  patterns: ['*']\n  update-types: [minor, patch]\nrustcrypto:\n  patterns: [sha2]\n",
    &["rustcrypto"]
)]
fn nonconforming_groups_fail(#[case] yaml: &str, #[case] exceptions: &[&str]) -> Result<()> {
    ensure!(
        assert_group_policy("cargo", &groups_from(yaml)?, exceptions).is_err(),
        "group policy should refuse {yaml:?}"
    );
    Ok(())
}
