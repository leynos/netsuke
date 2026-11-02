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
//!
//! # One table of observing entry points
//!
//! Seven adapters take the same argument shape — a path, the default policy,
//! an observer, and whichever seams they add — and differ only in which entry
//! point they call. Writing them as seven functions would repeat that shape
//! seven times, so each is one [`ObservedEntryPoint`] variant and the adapter
//! table below drives them all. The bare adapter, which takes no observer at
//! all, stays its own row, and so does the clock measurement, because neither
//! shares that shape.

use anyhow::Result;
use camino::Utf8Path;
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
    Capabilities, Case, StagedFixture, cases, environment, exhausted_budget, fixed_clock_config,
    one_var_reader, path_entry_point, path_entry_point_with_environment, settings,
};

/// Snapshot one row's document under the name `name`.
///
/// Every row here does the same three things: render the document its
/// capabilities and seam call for, bind the snapshot settings, and assert
/// against `name`. Stating that once keeps each row's own code to the entry
/// point it characterizes.
///
/// `name` is passed rather than derived from the test function, because three
/// pairs of entry points differ only by a suffix and a derived name would read
/// as the same row spelled twice.
///
/// # Errors
///
/// Returns an error when no fixture is applicable or one cannot be staged and
/// loaded.
fn record_row<F>(name: &str, capabilities: Capabilities, seam: StageSeam, load: F) -> Result<()>
where
    F: FnMut(&Case, &Utf8Path, &mut Vec<ManifestLoadStage>) -> Result<NetsukeManifest>,
{
    let document = document(capabilities, seam, load)?;
    settings().bind(|| insta::assert_snapshot!(name, document));
    Ok(())
}

/// A path adapter that accepts a stage observer, and the seams it adds.
///
/// Each variant names one public entry point; its doc comment states why that
/// row exists. [`ObservedEntryPoint::load`] is the single place the calls
/// appear, so a row states which adapter it characterizes rather than restating
/// the adapter's argument list.
#[derive(Clone, Copy, Debug)]
enum ObservedEntryPoint {
    /// The policy-taking adapter on the defaults.
    ///
    /// It exists so a caller that wants none of the other seams does not have
    /// to name them, and the row records that it behaves as the fuller
    /// adapters do once their defaults are supplied.
    Policy,
    /// The policy-taking adapter with explicit ceilings.
    ///
    /// The `_and_limits` name describes the seam, not the value: a caller that
    /// wants the ceilings the constants define passes them explicitly rather
    /// than omitting them. Passing the default here is what keeps this row from
    /// recording a limit that is a choice of this test rather than behaviour of
    /// the loader.
    PolicyAndLimits,
    /// The policy-taking adapter with an injected environment reader.
    PolicyAndEnv,
    /// The environment adapter with explicit ceilings at their defaults.
    PolicyAndEnvAndLimits,
    /// The adapter that takes the reader already paired with its policy.
    PolicyAndEnvironment,
    /// The fully parameterized adapter with every ceiling at its default.
    PolicyAndEnvironmentAndLimits,
    /// The same adapter with ceilings that exhaust before any manifest parses.
    ///
    /// Exhaustion is charged against the source bytes first, so a ceiling below
    /// the manifest's own text fails ahead of YAML parsing, Jinja evaluation,
    /// and rendering — before the observer is notified of anything past
    /// ingestion. It is its own row so the default-ceiling rows above cannot
    /// conflate the loader's ordinary behaviour with a limit this test chose.
    ExhaustedBudget,
}

impl ObservedEntryPoint {
    /// Call this entry point on `path`, forwarding `observer`.
    ///
    /// # Errors
    ///
    /// Returns an error when the entry point cannot load the manifest.
    fn load(
        self,
        path: &Utf8Path,
        observer: Option<&mut dyn FnMut(ManifestLoadStage)>,
    ) -> Result<NetsukeManifest> {
        match self {
            Self::Policy => from_path_with_policy(path, policy(), observer),
            Self::PolicyAndLimits => from_path_with_policy_and_limits(
                path,
                policy(),
                ManifestBudgetLimits::default(),
                observer,
            ),
            Self::PolicyAndEnv => {
                from_path_with_policy_and_env(path, policy(), &one_var_reader(), observer)
            }
            Self::PolicyAndEnvAndLimits => from_path_with_policy_and_env_and_limits(
                path,
                policy(),
                &one_var_reader(),
                ManifestBudgetLimits::default(),
                observer,
            ),
            Self::PolicyAndEnvironment => from_path_with_policy_and_environment(
                path,
                policy(),
                &environment(&one_var_reader()),
                observer,
            ),
            Self::PolicyAndEnvironmentAndLimits => {
                full_row(path, ManifestBudgetLimits::default(), observer)
            }
            Self::ExhaustedBudget => full_row(path, exhausted_budget(), observer),
        }
    }
}

/// Call the fully parameterized adapter with `budget_limits`.
///
/// The two rows over this entry point differ only in which ceilings they pass,
/// so the call itself is stated once here.
///
/// # Errors
///
/// Returns an error when the adapter cannot load the manifest.
fn full_row(
    path: &Utf8Path,
    budget_limits: ManifestBudgetLimits,
    observer: Option<&mut dyn FnMut(ManifestLoadStage)>,
) -> Result<NetsukeManifest> {
    from_path_with_policy_and_environment_and_limits(
        path,
        policy(),
        &environment(&one_var_reader()),
        budget_limits,
        RecipeShell::host_default(),
        observer,
    )
}

/// Snapshot what `from_path` produces.
///
/// `from_path` is the path entry point with no seam: it takes no observer
/// parameter at all, which this row records rather than leaving implicit. It
/// exists so a caller that wants none of the other seams does not have to name
/// them, and the row records that it behaves as the policy-taking entry point
/// does once the defaults are supplied.
#[rstest]
fn from_path_row() -> Result<()> {
    record_row(
        "from_path",
        path_entry_point(),
        StageSeam::Absent,
        |_, path, _| from_path(path),
    )
}

/// Snapshot every observer-taking path adapter.
///
/// One case per adapter, with the snapshot name written out: three pairs differ
/// only by a suffix, so a derived name would read as the same row twice.
#[rstest]
#[case::policy(
    "from_path_with_policy",
    ObservedEntryPoint::Policy,
    path_entry_point()
)]
#[case::policy_and_limits(
    "from_path_with_policy_and_limits",
    ObservedEntryPoint::PolicyAndLimits,
    path_entry_point()
)]
#[case::policy_and_env(
    "from_path_with_policy_and_env",
    ObservedEntryPoint::PolicyAndEnv,
    path_entry_point_with_environment()
)]
#[case::policy_and_env_and_limits(
    "from_path_with_policy_and_env_and_limits",
    ObservedEntryPoint::PolicyAndEnvAndLimits,
    path_entry_point_with_environment()
)]
#[case::policy_and_environment(
    "from_path_with_policy_and_environment",
    ObservedEntryPoint::PolicyAndEnvironment,
    path_entry_point_with_environment()
)]
#[case::policy_and_environment_and_limits(
    "from_path_with_policy_and_environment_and_limits",
    ObservedEntryPoint::PolicyAndEnvironmentAndLimits,
    path_entry_point_with_environment()
)]
#[case::exhausted_budget(
    "budget_exhaustion",
    ObservedEntryPoint::ExhaustedBudget,
    path_entry_point_with_environment()
)]
fn observed_path_row(
    #[case] name: &str,
    #[case] entry: ObservedEntryPoint,
    #[case] capabilities: Capabilities,
) -> Result<()> {
    record_row(
        name,
        capabilities,
        StageSeam::Accepted,
        |_, path, stages| entry.load(path, Some(&mut |stage| stages.push(stage))),
    )
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
