//! Operator-authority tests for the adopted user-scope chain.
//!
//! A user candidate is normally a home or XDG location, but the same path can
//! also be the project configuration file: running `netsuke` in `$HOME` makes
//! `$HOME/.netsuke.toml` both. The user pass must apply the identity test the
//! dependency scan applies, so adopting a user file cannot upgrade a path the
//! project may control into an operator-authority root.

use super::probe::{Answer, ScriptedProbe};
use super::{Case, HomeArrangement, write_config};
use crate::cli::discovery::discovery_env_source;
use crate::cli::discovery::layers::ProjectKey;
use crate::cli::discovery::layers::user_candidate::{
    AutomaticLayers, CandidateEntry, UserScopeTrace, automatic_base_layers_with_probe,
};
use crate::cli::discovery::paths::FsPathNormalizer;
use crate::cli::test_support::TestEnv;
use anyhow::{Context, Result, ensure};
use rstest::rstest;
use std::path::Path;

/// Run the automatic pass for `env`, classifying `project_file` as project scope.
fn layers_with_project_key(
    env: &TestEnv,
    project_file: &Path,
) -> ortho_config::OrthoResult<AutomaticLayers> {
    // The user file is present, so the loader answers before the presence probe
    // is consulted; the probe only has to exist.
    let probe = ScriptedProbe::new(Answer::Entry(CandidateEntry::Absent));
    let project_key = ProjectKey::classify(&FsPathNormalizer, project_file);
    automatic_base_layers_with_probe(&probe, None, discovery_env_source(env), Some(&project_key))
}

/// A user file that is also the project file cannot donate operator authority.
///
/// The dependency scan withholds authority from the project file wherever it is
/// read from. The user pass loads the same path first, so returning a trusted
/// root unconditionally would grant operator scope to project-controlled
/// content — the precise downgrade the identity test exists to prevent.
#[rstest]
fn user_file_that_is_the_project_file_is_untrusted() -> Result<()> {
    let case = Case::new()?;
    let user_file = write_config(&case.home().join(".netsuke.toml"), "jobs = 4\n")?;

    let found = layers_with_project_key(
        &case.environment(HomeArrangement::EmptyXdgConfigHome),
        &user_file,
    )
    .context("the user file should still be adopted")?;

    ensure!(
        !found.layers.is_empty(),
        "the adopted chain should still supply its layers"
    );
    ensure!(
        matches!(found.user_scope, UserScopeTrace::Adopted { .. }),
        "the rejected-authority candidate should still be reported as adopted, got {:?}",
        found.user_scope
    );
    ensure!(
        !found.is_trusted_root,
        "a user candidate that is the project file must not donate operator authority"
    );
    Ok(())
}

/// A user file distinct from the project file keeps operator authority.
///
/// Without this case, an implementation that withheld trust from every user
/// candidate would satisfy the rejection test above while silently downgrading
/// ordinary user configuration.
#[rstest]
fn user_file_distinct_from_the_project_file_stays_trusted() -> Result<()> {
    let case = Case::new()?;
    let user_file = write_config(&case.home().join(".netsuke.toml"), "jobs = 4\n")?;
    let project_dir = case.base("elsewhere")?;
    let project_file = write_config(&project_dir.join(".netsuke.toml"), "jobs = 9\n")?;

    let found = layers_with_project_key(
        &case.environment(HomeArrangement::EmptyXdgConfigHome),
        &project_file,
    )
    .context("the user file should be adopted")?;

    ensure!(
        found.layers.iter().any(|layer| layer.path().is_some()),
        "the adopted chain should name its file layer"
    );
    ensure!(
        found.is_trusted_root,
        "a user candidate that is not the project file must keep operator authority"
    );
    ensure!(
        user_file != project_file,
        "the fixture must keep the two paths distinct"
    );
    Ok(())
}
