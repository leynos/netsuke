//! What each public loader entry point produces today.
//!
//! Eleven public entry points load a manifest, and they differ only in how much
//! of the loader's seam they accept: a network policy, an environment reader or
//! a paired environment, resource ceilings, a stdlib configuration, or a stage
//! observer. The refactor that replaces their shared core must leave every one
//! of them producing exactly what it produces now, and the only trustworthy
//! oracle for that claim is the code before the refactor — so these snapshots
//! are recorded against the unchanged loader and re-checked at every later
//! milestone.
//!
//! Comparing a wrapper against the core *after* the refactor would be equal by
//! construction and would prove nothing. That is why the baseline is recorded
//! first and never re-accepted afterwards.
//!
//! # One row, one document
//!
//! A row snapshots one document covering every fixture it may load, rather than
//! one snapshot per fixture. The directory stays small enough to hold in view,
//! a reviewer reads one file to see whether a refactor changed *anything* about
//! an entry point, and the documents are diffable against each other — which is
//! what turns "these wrappers delegate identically" from an assertion into an
//! observation.
//!
//! Each snapshot name is written out rather than derived from the test
//! function. Three pairs of entry points differ only in a suffix (`_and_env`
//! against `_and_environment`, and the `_and_limits` forms of both), so a
//! derived name would read as the same row spelled twice.
//!
//! # Applicability and the clock
//!
//! What each row may load is decided by [`Case::is_loadable`] against the
//! loader's [`Capabilities`], and a row that finds no applicable fixture fails
//! rather than snapshotting an empty document. Without that guard a mistake in
//! the capability table would silently reduce an entry point's characterization
//! to nothing while the snapshot stayed green.
//!
//! Only the entries that accept a [`StdlibConfig`] can pin the clock, so
//! `clock_now` is loaded only there. That is a property of the public surface
//! rather than a choice: a path entry point builds its own configuration from
//! the workspace it opens and has no seam for a caller's clock, so
//! [`path_entry_point_cannot_inject_a_clock`] measures that rather than leaving
//! it as an assumption the capability table happens to encode.

use anyhow::Result;
use camino::Utf8Path;
use rstest::rstest;

use netsuke::ast::NetsukeManifest;
use netsuke::manifest::{
    EnvAccessPolicy, ManifestLoadStage, from_path, from_path_with_policy,
    from_path_with_policy_and_env, from_path_with_policy_and_env_and_limits,
    from_path_with_policy_and_environment, from_path_with_policy_and_environment_and_limits,
    from_path_with_policy_and_limits, from_str, from_str_with_env, from_str_with_env_and_config,
    from_str_with_env_and_policy,
};
use netsuke::recipe_shell::RecipeShell;
use netsuke::stdlib::NetworkPolicy;

use super::fixtures::{
    Capabilities, Case, StagedFixture, cases, environment, exhausted_budget, fixed_clock_config,
    injected_environment, injected_environment_and_clock, one_var_reader, path_entry_point,
    path_entry_point_with_environment, process_environment, report, settings,
};

/// Return the fixtures a loader with `capabilities` may load.
fn applicable(capabilities: Capabilities) -> Vec<Case> {
    cases()
        .iter()
        .copied()
        .filter(|case| case.is_loadable(capabilities))
        .collect()
}

/// Render the document a row snapshots, staging each fixture in turn.
///
/// `load` receives the fixture and the path a path entry point should read: the
/// staged copy, which lives in a fresh temporary workspace.
///
/// The observer records every stage the entry point notifies, which is part of
/// what these rows characterize — an entry point that accepts a stage observer
/// is called with one, so a refactor that stops notifying changes what a real
/// caller sees. The stages themselves are not snapshotted: they are identical
/// for every fixture and would only pad the document.
///
/// # Errors
///
/// Returns an error when the capability table admits no fixture — the row would
/// snapshot an empty document and characterize nothing — or when a fixture
/// cannot be staged or read.
fn document<F>(capabilities: Capabilities, mut load: F) -> Result<String>
where
    F: FnMut(&Case, &Utf8Path, &mut Vec<ManifestLoadStage>) -> Result<NetsukeManifest>,
{
    let applicable = applicable(capabilities);
    anyhow::ensure!(
        !applicable.is_empty(),
        "no fixture is applicable to this entry point; the capability table is wrong"
    );
    let mut stages = Vec::new();
    let mut blocks = Vec::with_capacity(applicable.len());
    for case in applicable {
        let fixture = StagedFixture::new(&case)?;
        let outcome = load(&case, fixture.manifest_path(), &mut stages);
        blocks.push(report(&[(case.label.to_owned(), outcome)]));
    }
    Ok(blocks.join("\n\n"))
}

/// Render the document a row snapshots for a string entry point.
///
/// A string load names the manifest only for diagnostics, so the fixture's own
/// repository-relative path serves as that name. Naming it anything else would
/// suggest the loader had been handed a workspace it does not have.
///
/// # Errors
///
/// Returns an error under the same conditions as [`document`], plus when a
/// fixture cannot be read.
fn document_from_string<F>(capabilities: Capabilities, mut load: F) -> Result<String>
where
    F: FnMut(&str) -> Result<NetsukeManifest>,
{
    document(capabilities, |case, _path, _stages| {
        load(&case.manifest_text()?)
    })
}

/// Return the policy every policy-taking row passes.
///
/// `NetworkPolicy::default` is HTTPS-only. No applicable fixture reaches the
/// network: `uses_fetch` addresses `http://127.0.0.1:1/`, which the policy
/// refuses at the scheme check before a socket is opened, and only a
/// refusing-surface row loads it. The policy is therefore not what any row
/// characterizes; it is the value an ordinary caller supplies.
fn policy() -> NetworkPolicy {
    NetworkPolicy::default()
}

/// Return the permissive access policy the adapters default to.
fn permissive() -> EnvAccessPolicy {
    EnvAccessPolicy::default()
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

/// Snapshot what `from_str` produces.
///
/// `from_str` takes no seam at all: it reads the process environment and the
/// host clock. It is therefore the narrowest row, and the fixtures it may load
/// are exactly the ones no seam can change.
#[rstest]
fn from_str_row() -> Result<()> {
    let document = document_from_string(process_environment(), from_str)?;
    settings().bind(|| insta::assert_snapshot!("from_str", document));
    Ok(())
}

#[rstest]
fn from_str_with_env_row() -> Result<()> {
    let document = document_from_string(injected_environment(), |text| {
        from_str_with_env(text, &one_var_reader())
    })?;
    settings().bind(|| insta::assert_snapshot!("from_str_with_env", document));
    Ok(())
}

#[rstest]
fn from_str_with_env_and_policy_row() -> Result<()> {
    let document = document_from_string(injected_environment(), |text| {
        from_str_with_env_and_policy(text, &one_var_reader(), &permissive())
    })?;
    settings().bind(|| {
        insta::assert_snapshot!("from_str_with_env_and_policy", document);
    });
    Ok(())
}

#[rstest]
fn from_str_with_env_and_config_row() -> Result<()> {
    let document = document_from_string(injected_environment_and_clock(), |text| {
        from_str_with_env_and_config(text, &one_var_reader(), fixed_clock_config()?)
    })?;
    settings().bind(|| {
        insta::assert_snapshot!("from_str_with_env_and_config", document);
    });
    Ok(())
}

/// Snapshot what `from_path` produces.
///
/// `from_path` is the path entry point with no seam: no policy, no budget, no
/// observer. It exists so a caller that wants none of them does not have to
/// name them, and the row records that it behaves as the policy-taking entry
/// point does once the defaults are supplied.
#[rstest]
fn from_path_row() -> Result<()> {
    let document = document(path_entry_point(), |_, path, _| from_path(path))?;
    settings().bind(|| insta::assert_snapshot!("from_path", document));
    Ok(())
}

#[rstest]
fn from_path_with_policy_row() -> Result<()> {
    let document = document(path_entry_point(), |_, path, stages| {
        from_path_with_policy(path, policy(), Some(&mut |stage| stages.push(stage)))
    })?;
    settings().bind(|| insta::assert_snapshot!("from_path_with_policy", document));
    Ok(())
}

#[rstest]
fn from_path_with_policy_and_limits_row() -> Result<()> {
    let document = document(path_entry_point(), |_, path, stages| {
        from_path_with_policy_and_limits(
            path,
            policy(),
            exhausted_budget(),
            Some(&mut |stage| stages.push(stage)),
        )
    })?;
    settings().bind(|| {
        insta::assert_snapshot!("from_path_with_policy_and_limits", document);
    });
    Ok(())
}

#[rstest]
fn from_path_with_policy_and_env_row() -> Result<()> {
    let document = document(path_entry_point_with_environment(), |_, path, stages| {
        from_path_with_policy_and_env(
            path,
            policy(),
            &one_var_reader(),
            Some(&mut |stage| stages.push(stage)),
        )
    })?;
    settings().bind(|| {
        insta::assert_snapshot!("from_path_with_policy_and_env", document);
    });
    Ok(())
}

#[rstest]
fn from_path_with_policy_and_env_and_limits_row() -> Result<()> {
    let document = document(path_entry_point_with_environment(), |_, path, stages| {
        from_path_with_policy_and_env_and_limits(
            path,
            policy(),
            &one_var_reader(),
            exhausted_budget(),
            Some(&mut |stage| stages.push(stage)),
        )
    })?;
    settings().bind(|| {
        insta::assert_snapshot!("from_path_with_policy_and_env_and_limits", document);
    });
    Ok(())
}

#[rstest]
fn from_path_with_policy_and_environment_row() -> Result<()> {
    let document = document(path_entry_point_with_environment(), |_, path, stages| {
        from_path_with_policy_and_environment(
            path,
            policy(),
            &environment(&one_var_reader()),
            Some(&mut |stage| stages.push(stage)),
        )
    })?;
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
/// omitting them. The exhausted budget is exercised by
/// [`from_path_with_policy_and_limits_row`] instead, so this row still shows a
/// successful load.
#[rstest]
fn from_path_with_policy_and_environment_and_limits_row() -> Result<()> {
    let document = document(path_entry_point_with_environment(), |_, path, stages| {
        from_path_with_policy_and_environment_and_limits(
            path,
            policy(),
            &environment(&one_var_reader()),
            exhausted_budget(),
            RecipeShell::host_default(),
            Some(&mut |stage| stages.push(stage)),
        )
    })?;
    settings().bind(|| {
        insta::assert_snapshot!("from_path_with_policy_and_environment_and_limits", document);
    });
    Ok(())
}
