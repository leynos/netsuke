//! The rows that characterize the workspace-opening entry points.
//!
//! Every entry point here takes a manifest *path* and opens the workspace that
//! holds it, which is what separates these rows from the string rows in the
//! parent module: a path load can only read what its staged workspace contains,
//! so each fixture is copied in by [`StagedFixture`] before the load.
//!
//! This is a submodule rather than a third top-level suite because the rows
//! share the parent's harness — its staging, its capability table, and its
//! [`document`](super::document) renderer — and a sibling suite would have to
//! restate all three.

use anyhow::Result;
use rstest::rstest;

use netsuke::ast::NetsukeManifest;
use netsuke::manifest::{
    ManifestBudgetLimits, ManifestLoadStage, from_path, from_path_with_policy,
    from_path_with_policy_and_env, from_path_with_policy_and_env_and_limits,
    from_path_with_policy_and_environment, from_path_with_policy_and_environment_and_limits,
    from_path_with_policy_and_limits, from_str_with_env_and_config,
};
use netsuke::recipe_shell::RecipeShell;

use super::{StageSeam, document, policy};
use crate::manifest_load_options::fixtures::{
    StagedFixture, cases, environment, exhausted_budget, fixed_clock_config, one_var_reader,
    path_entry_point, path_entry_point_with_environment, settings,
};

/// Snapshot what `from_path` produces.
///
/// `from_path` is the path entry point with no seam: it takes no observer
/// parameter at all, which this row records rather than leaving implicit. It
/// exists so a caller that wants none of the other seams does not have to name
/// them, and the row records that it behaves as the policy-taking entry point
/// does once the defaults are supplied.
#[rstest]
fn from_path_row() -> Result<()> {
    let document = document(path_entry_point(), StageSeam::Absent, |_, path, _| {
        from_path(path)
    })?;
    settings().bind(|| insta::assert_snapshot!("from_path", document));
    Ok(())
}

#[rstest]
fn from_path_with_policy_row() -> Result<()> {
    let document = document(
        path_entry_point(),
        StageSeam::Accepted,
        |_, path, stages| {
            from_path_with_policy(path, policy(), Some(&mut |stage| stages.push(stage)))
        },
    )?;
    settings().bind(|| insta::assert_snapshot!("from_path_with_policy", document));
    Ok(())
}

#[rstest]
fn from_path_with_policy_and_limits_row() -> Result<()> {
    let document = document(
        path_entry_point(),
        StageSeam::Accepted,
        |_, path, stages| {
            from_path_with_policy_and_limits(
                path,
                policy(),
                ManifestBudgetLimits::default(),
                Some(&mut |stage| stages.push(stage)),
            )
        },
    )?;
    settings().bind(|| {
        insta::assert_snapshot!("from_path_with_policy_and_limits", document);
    });
    Ok(())
}

#[rstest]
fn from_path_with_policy_and_env_row() -> Result<()> {
    let document = document(
        path_entry_point_with_environment(),
        StageSeam::Accepted,
        |_, path, stages| {
            from_path_with_policy_and_env(
                path,
                policy(),
                &one_var_reader(),
                Some(&mut |stage| stages.push(stage)),
            )
        },
    )?;
    settings().bind(|| {
        insta::assert_snapshot!("from_path_with_policy_and_env", document);
    });
    Ok(())
}

#[rstest]
fn from_path_with_policy_and_env_and_limits_row() -> Result<()> {
    let document = document(
        path_entry_point_with_environment(),
        StageSeam::Accepted,
        |_, path, stages| {
            from_path_with_policy_and_env_and_limits(
                path,
                policy(),
                &one_var_reader(),
                ManifestBudgetLimits::default(),
                Some(&mut |stage| stages.push(stage)),
            )
        },
    )?;
    settings().bind(|| {
        insta::assert_snapshot!("from_path_with_policy_and_env_and_limits", document);
    });
    Ok(())
}

#[rstest]
fn from_path_with_policy_and_environment_row() -> Result<()> {
    let document = document(
        path_entry_point_with_environment(),
        StageSeam::Accepted,
        |_, path, stages| {
            from_path_with_policy_and_environment(
                path,
                policy(),
                &environment(&one_var_reader()),
                Some(&mut |stage| stages.push(stage)),
            )
        },
    )?;
    settings().bind(|| {
        insta::assert_snapshot!("from_path_with_policy_and_environment", document);
    });
    Ok(())
}

/// Snapshot the fully parameterized entry point with every ceiling at its
/// default.
///
/// The `_and_limits` name describes the seam, not the value: a caller that
/// wants the ceilings the constants define passes them explicitly rather than
/// omitting them. Exhaustion is a separate row below, which is what keeps this
/// one from recording a limit that is a choice of this test rather than
/// behaviour of the loader.
#[rstest]
fn from_path_with_policy_and_environment_and_limits_row() -> Result<()> {
    let document = document(
        path_entry_point_with_environment(),
        StageSeam::Accepted,
        |_, path, stages| {
            from_path_with_policy_and_environment_and_limits(
                path,
                policy(),
                &environment(&one_var_reader()),
                ManifestBudgetLimits::default(),
                RecipeShell::host_default(),
                Some(&mut |stage| stages.push(stage)),
            )
        },
    )?;
    settings().bind(|| {
        insta::assert_snapshot!("from_path_with_policy_and_environment_and_limits", document);
    });
    Ok(())
}

/// Snapshot the budget-exhaustion path of the fullest entry point.
///
/// Exhaustion is charged against the source bytes first, so a ceiling below the
/// manifest's own text fails ahead of YAML parsing and rendering — before the
/// stage observer is notified of anything past ingestion. Recording it as its
/// own row is what lets the rows above leave the default ceilings in place: a
/// limit a test chose would otherwise be entangled with the entry point's
/// ordinary behaviour, and a reviewer could not tell which of the two moved.
///
/// The other path entry points forward their ceilings to this one, so a
/// separate exhaustion row per adapter would record the same behaviour under a
/// different name.
#[rstest]
fn budget_exhaustion_row() -> Result<()> {
    let document = document(
        path_entry_point_with_environment(),
        StageSeam::Accepted,
        |_, path, stages| {
            from_path_with_policy_and_environment_and_limits(
                path,
                policy(),
                &environment(&one_var_reader()),
                exhausted_budget(),
                RecipeShell::host_default(),
                Some(&mut |stage| stages.push(stage)),
            )
        },
    )?;
    settings().bind(|| {
        insta::assert_snapshot!("budget_exhaustion", document);
    });
    Ok(())
}

/// Assert that a path entry point consults the host clock and cannot be given
/// another.
///
/// The capability table gives every path row no clock seam, and this row is the
/// measurement behind that claim. Without it, a later change that *did* give a
/// path entry point a clock seam would leave the clock fixture silently
/// excluded from seven rows, where the omission would read as a decision rather
/// than a gap.
///
/// The same manifest is loaded twice: once through a string entry that can pin
/// the clock, once through a path entry that cannot. Equal descriptions would
/// mean the path entry had somehow obtained the pinned instant, so it must
/// differ — and it differs by rendering the host's, which the fixed instant is
/// a decade away from.
#[rstest]
fn path_entry_point_cannot_inject_a_clock() -> Result<()> {
    let case = cases()
        .iter()
        .copied()
        .find(|case| case.hazards.consults_clock)
        .ok_or_else(|| anyhow::anyhow!("the corpus no longer holds a clock-reading fixture"))?;
    anyhow::ensure!(
        !case.is_loadable(path_entry_point()),
        "a path entry point now admits the clock fixture; give the path rows an injected clock"
    );

    let text = case.manifest_text()?;
    let pinned = from_str_with_env_and_config(&text, &one_var_reader(), fixed_clock_config()?)?;
    let staged = StagedFixture::new(&case)?;
    let mut stages = Vec::new();
    let host = from_path_with_policy(
        staged.manifest_path(),
        policy(),
        Some(&mut |stage| stages.push(stage)),
    )?;

    anyhow::ensure!(
        stages
            == [
                ManifestLoadStage::ManifestIngestion,
                ManifestLoadStage::InitialYamlParsing,
                ManifestLoadStage::TemplateExpansion,
                ManifestLoadStage::FinalRendering,
            ],
        "a successful path load notified {stages:?} rather than every stage"
    );
    anyhow::ensure!(
        describe(&pinned)? != describe(&host)?,
        "a path entry point rendered the pinned clock; it has gained a clock seam \
         and the path rows must start using it"
    );
    Ok(())
}

/// Return the first target's rendered description.
///
/// # Errors
///
/// Returns an error when the manifest has no target, or when the fixture's
/// description does not survive as a string.
fn describe(manifest: &NetsukeManifest) -> Result<String> {
    let target = manifest
        .targets
        .first()
        .ok_or_else(|| anyhow::anyhow!("the clock fixture has no target"))?;
    target
        .description
        .clone()
        .ok_or_else(|| anyhow::anyhow!("the clock fixture's target has no description"))
}
