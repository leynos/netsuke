//! Dependabot group policy: one minor-and-patch catch-all per ecosystem.
//!
//! Routine bumps arrive as one grouped pull request per ecosystem, while each
//! major update arrives on its own. A narrow lockstep group, such as the
//! `RustCrypto` family, is the only permitted exception: it must exist with
//! exactly its family's patterns and no `update-types` limit, and it must
//! precede the catch-all because Dependabot assigns a dependency to the first
//! group whose patterns match it.

use anyhow::{Result, ensure};
use indexmap::IndexMap;
use rstest::rstest;
use serde::Deserialize;

/// Parsed Dependabot group definition.
///
/// Unknown options are refused: `exclude-patterns`, `applies-to` or
/// `group-by` would each change which updates a group takes while its
/// `patterns` still read as a catch-all.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct DependabotGroup {
    #[serde(default)]
    patterns: Vec<String>,
    #[serde(default)]
    update_types: Vec<String>,
}

/// A lockstep family permitted to group its majors, with its exact patterns.
pub(crate) struct LockstepGroup<'a> {
    /// Group name in `.github/dependabot.yml`.
    pub(crate) name: &'a str,
    /// The family's patterns, in file order.
    pub(crate) patterns: &'a [&'a str],
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

/// Assert that an ecosystem has one trailing minor-and-patch catch-all group,
/// every listed lockstep group exactly as defined, and no other group.
///
/// # Errors
///
/// Returns an error naming the ecosystem when the catch-all is missing,
/// duplicated, or not listed last, when a lockstep group is missing or
/// differs from its definition, or when any other group would take majors.
pub(crate) fn assert_group_policy(
    ecosystem: &str,
    groups: &IndexMap<String, DependabotGroup>,
    lockstep: &[LockstepGroup<'_>],
) -> Result<()> {
    let catch_all = sole_catch_all(ecosystem, groups)?;
    for family in lockstep {
        assert_lockstep_group(ecosystem, groups, family)?;
    }
    let unexpected = groups
        .keys()
        .map(String::as_str)
        .filter(|name| *name != catch_all && !lockstep.iter().any(|family| family.name == *name))
        .collect::<Vec<_>>();
    ensure!(
        unexpected.is_empty(),
        "{ecosystem} groups {unexpected:?} would group major updates; majors must arrive ungrouped"
    );
    ensure!(
        groups.keys().last().map(String::as_str) == Some(catch_all),
        "{ecosystem} catch-all group must be listed last so lockstep groups claim their members first"
    );
    Ok(())
}

/// Return the name of the ecosystem's single minor-and-patch catch-all group.
///
/// # Errors
///
/// Returns an error when there is no such group or more than one.
fn sole_catch_all<'g>(
    ecosystem: &str,
    groups: &'g IndexMap<String, DependabotGroup>,
) -> Result<&'g str> {
    let catch_alls = groups
        .iter()
        .filter(|(_, group)| is_minor_and_patch_catch_all(group))
        .map(|(name, _)| name.as_str())
        .collect::<Vec<_>>();
    match catch_alls.as_slice() {
        [only] => Ok(only),
        _ => Err(anyhow::anyhow!(
            "{ecosystem} should define exactly one minor-and-patch catch-all group, found {catch_alls:?}"
        )),
    }
}

/// Assert that a lockstep group exists with exactly its family's patterns and
/// no `update-types` limit, so it keeps the family's majors together.
///
/// # Errors
///
/// Returns an error when the group is missing, matches a different set of
/// dependencies, or is limited to some update types.
fn assert_lockstep_group(
    ecosystem: &str,
    groups: &IndexMap<String, DependabotGroup>,
    family: &LockstepGroup<'_>,
) -> Result<()> {
    let name = family.name;
    let group = groups
        .get(name)
        .ok_or_else(|| anyhow::anyhow!("{ecosystem} should define the {name} lockstep group"))?;
    ensure!(
        group.patterns == family.patterns,
        "{ecosystem} {name} group should match exactly {:?}, found {:?}",
        family.patterns,
        group.patterns
    );
    ensure!(
        group.update_types.is_empty(),
        "{ecosystem} {name} group should not limit update-types, found {:?}",
        group.update_types
    );
    Ok(())
}

/// Parse a YAML `groups` mapping fixture.
fn groups_from(yaml: &str) -> Result<IndexMap<String, DependabotGroup>> {
    Ok(serde_yaml::from_str(yaml)?)
}

/// The lockstep family used by the fixtures below.
const SHA2_FAMILY: &[LockstepGroup<'static>] = &[LockstepGroup {
    name: "rustcrypto",
    patterns: &["sha2"],
}];

/// A lone catch-all, and a lockstep group listed before it, both conform.
#[rstest]
#[case::catch_all_only("all:\n  patterns: ['*']\n  update-types: [minor, patch]\n", &[])]
#[case::lockstep_first(
    "rustcrypto:\n  patterns: [sha2]\nall:\n  patterns: ['*']\n  update-types: [patch, minor]\n",
    SHA2_FAMILY
)]
fn conforming_groups_pass(
    #[case] yaml: &str,
    #[case] lockstep: &[LockstepGroup<'static>],
) -> Result<()> {
    assert_group_policy("cargo", &groups_from(yaml)?, lockstep)
}

/// Each way of letting majors into a group, of losing the catch-all, or of
/// losing or widening a lockstep group, is refused.
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
    SHA2_FAMILY
)]
#[case::lockstep_missing(
    "all:\n  patterns: ['*']\n  update-types: [minor, patch]\n",
    SHA2_FAMILY
)]
#[case::lockstep_widened(
    "rustcrypto:\n  patterns: ['*']\nall:\n  patterns: ['*']\n  update-types: [minor, patch]\n",
    SHA2_FAMILY
)]
#[case::lockstep_limited(
    "rustcrypto:\n  patterns: [sha2]\n  update-types: [minor, patch]\nall:\n  patterns: ['*']\n  update-types: [minor, patch]\n",
    SHA2_FAMILY
)]
fn nonconforming_groups_fail(
    #[case] yaml: &str,
    #[case] lockstep: &[LockstepGroup<'static>],
) -> Result<()> {
    ensure!(
        assert_group_policy("cargo", &groups_from(yaml)?, lockstep).is_err(),
        "group policy should refuse {yaml:?}"
    );
    Ok(())
}

/// Options that narrow a group's reach are refused when the file is parsed.
#[rstest]
#[case::exclude_patterns(
    "all:\n  patterns: ['*']\n  exclude-patterns: [serde]\n  update-types: [minor, patch]\n"
)]
#[case::applies_to_security(
    "all:\n  patterns: ['*']\n  applies-to: security-updates\n  update-types: [minor, patch]\n"
)]
#[case::group_by("all:\n  group-by: dependency-name\n")]
fn narrowing_options_fail_to_parse(#[case] yaml: &str) -> Result<()> {
    ensure!(
        groups_from(yaml).is_err(),
        "group model should refuse {yaml:?}"
    );
    Ok(())
}
