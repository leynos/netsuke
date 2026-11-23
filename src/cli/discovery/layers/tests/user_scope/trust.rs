//! Operator-authority tests for the user-scope candidate test.
//!
//! A user candidate is normally a home or XDG location, but the same path can
//! also be the project configuration file: running `netsuke` in `$HOME` makes
//! `$HOME/.netsuke.toml` both. The user pass must not adopt such a candidate.
//! Adopting it would both grant operator authority to project-controlled
//! content and return before the system layers load, so the restrictions a
//! system file sets would stop applying.

use super::probe::{Answer, ScriptedProbe};
use super::{Case, HomeArrangement, canonical, write_config};
use crate::cli::discovery::discovery_env_source;
use crate::cli::discovery::layers::user_candidate::{
    AutomaticLayers, CandidateEntry, UserScopeTrace, automatic_base_layers_with_probe,
};
use crate::cli::discovery::layers::{ProjectIdentity, ProjectKey};
use crate::cli::discovery::paths::{FailingPathNormalizer, FsPathNormalizer, PathNormalizer};
use crate::cli::test_support::TestEnv;
use anyhow::{Context, Result, ensure};
use rstest::rstest;
use std::path::Path;

/// Return the canonical form of every layer path, for comparison.
fn layer_paths(found: &AutomaticLayers) -> Result<Vec<String>> {
    found
        .layers
        .iter()
        .filter_map(|layer| layer.path())
        .map(|path| canonical(Path::new(path.as_str())))
        .collect()
}

/// Run the automatic pass for `env`, classifying `project_file` as project scope.
///
/// The directory is left unnamed so the dependency scan reads only the operator
/// locations the case configured; the cases that need the project root pass it
/// explicitly.
fn layers_with_project_key(
    env: &TestEnv,
    project_file: &Path,
) -> ortho_config::OrthoResult<AutomaticLayers> {
    run_with_project_key(env, project_file, None)
}

/// Run the automatic pass against an explicit project `directory`.
fn run_with_project_key(
    env: &TestEnv,
    project_file: &Path,
    directory: Option<&Path>,
) -> ortho_config::OrthoResult<AutomaticLayers> {
    let project_key = ProjectKey::classify(&FsPathNormalizer, project_file);
    run_with_normalizer(env, &FsPathNormalizer, Some(&project_key), directory)
}

/// Run the automatic pass with an explicit normalizer and project key.
///
/// Passing them separately lets a case classify the key through the real
/// filesystem while candidate comparisons use a different resolution seam,
/// which is how an unresolvable *candidate* is isolated from an ambiguous key.
fn run_with_normalizer(
    env: &TestEnv,
    normalizer: &impl PathNormalizer,
    project_key: Option<&ProjectKey>,
    directory: Option<&Path>,
) -> ortho_config::OrthoResult<AutomaticLayers> {
    // The user file is present, so the loader answers before the presence probe
    // is consulted; the probe only has to exist.
    let probe = ScriptedProbe::new(Answer::Entry(CandidateEntry::Absent));
    automatic_base_layers_with_probe(
        &probe,
        directory,
        discovery_env_source(env),
        ProjectIdentity::new(normalizer, project_key),
    )
}

/// A home dotfile that is also the project file is not adopted.
///
/// The dependency scan withholds authority from the project file wherever it is
/// read from, and adopting the same path first would skip the system layers
/// entirely. Running from `$HOME` is what makes the two paths identical, so this
/// is the layout in which an early return would drop a system `env_block_var` or
/// `fetch_default_deny` restriction.
#[rstest]
fn project_file_candidate_is_not_adopted() -> Result<()> {
    let case = Case::new()?;
    let user_file = write_config(&case.home().join(".netsuke.toml"), "jobs = 4\n")?;

    let found = layers_with_project_key(
        &case.environment(HomeArrangement::EmptyXdgConfigHome),
        &user_file,
    )
    .context("the project file must not fail discovery")?;

    ensure!(
        found.user_scope == UserScopeTrace::Fallback,
        "a candidate that is the project file must not be adopted, got {:?}",
        found.user_scope
    );
    ensure!(
        found.is_trusted_root,
        "the fallback chain's root is the system file, which is operator scope"
    );
    ensure!(
        !found.layers.is_empty(),
        "the dependency scan should still supply the system layer"
    );
    Ok(())
}

/// Skipping the project-scoped candidate must not drop the system layer.
///
/// The fallback is only correct if the dependency scan composes the system file;
/// the composition boundary then appends the project chain, which the
/// `config_discovery_env_provenance` integration case exercises end to end.
/// Without the system layer, every restriction that exists only in the system
/// file — `env_block_var`, `fetch_default_deny` — silently stops applying
/// whenever Netsuke runs from the directory that holds the project file.
#[rstest]
fn project_file_candidate_keeps_the_system_layer() -> Result<()> {
    let case = Case::new()?;
    let home = case.home().to_path_buf();
    let user_file = write_config(&home.join(".netsuke.toml"), "jobs = 4\n")?;

    let found = run_with_project_key(
        &case.environment(HomeArrangement::EmptyXdgConfigHome),
        &user_file,
        Some(&home),
    )
    .context("the project file must not fail discovery")?;

    let paths = layer_paths(&found)?;
    let system = canonical(&case.system_config())?;
    ensure!(
        paths.contains(&system),
        "the system layer must still load so its restrictions keep applying: {paths:?}"
    );
    ensure!(
        found.user_scope == UserScopeTrace::Fallback,
        "the refused project file must leave the dependency scan to supply the base chain, got {:?}",
        found.user_scope
    );
    Ok(())
}

/// A user candidate that resolves to the project file is not adopted.
///
/// Candidate paths keep the spelling their scope produced while the project key
/// holds a resolved path, so a home directory reached through a link describes
/// the same file as the project configuration without comparing equal as text.
/// The identity test must resolve the candidate before comparing, or the linked
/// candidate would be adopted and the system layers skipped.
#[cfg(unix)]
#[rstest]
fn project_file_candidate_reached_through_a_link_is_not_adopted() -> Result<()> {
    let case = Case::new()?;
    // The target is relative so the link stays inside `$HOME`: the loader reads
    // a candidate through a capability on its parent directory and rejects an
    // absolute link escaping it outright.
    let project_dir = case.home().join("linked-project");
    test_support::fs::create_dir(&project_dir).context("create the project directory")?;
    let project_file = write_config(&project_dir.join(".netsuke.toml"), "jobs = 9\n")?;
    let user_link = case.home().join(".netsuke.toml");
    test_support::fs::symlink(Path::new("linked-project/.netsuke.toml"), &user_link)
        .context("link the user candidate at the project file")?;

    let found = layers_with_project_key(
        &case.environment(HomeArrangement::EmptyXdgConfigHome),
        &project_file,
    )
    .context("the linked candidate must not fail discovery")?;

    ensure!(
        found.user_scope == UserScopeTrace::Fallback,
        "a candidate that resolves to the project file must not be adopted, got {:?}",
        found.user_scope
    );
    Ok(())
}

/// A distinct dotfile in the refused candidate's base supplies the chain.
///
/// `XDG_CONFIG_HOME` contributes `netsuke/config.toml` before `.netsuke.toml`,
/// and classification resolves links, so a nested spelling that links to the
/// project file is refused. The refusal is decisive for the user pass, but the
/// dependency's walk resumes at the next candidate, and the base's distinct
/// dotfile loads before any system directory. The system layer therefore does
/// not supply the chain in this layout, which is why the documented rule is
/// the first later candidate that loads rather than the system file.
#[cfg(unix)]
#[rstest]
fn distinct_dotfile_in_the_refused_base_supplies_the_fallback_chain() -> Result<()> {
    let case = Case::new()?;
    let base = case.base("xdg")?;
    let project_dir = base.join("netsuke");
    test_support::fs::create_dir(&project_dir).context("create the project directory")?;
    let project_file = write_config(&project_dir.join(".netsuke.toml"), "jobs = 9\n")?;
    // The target is relative so the link stays inside the candidate's parent
    // directory: the loader reads a candidate through a capability on it and
    // rejects an absolute link escaping that directory outright.
    test_support::fs::symlink(Path::new(".netsuke.toml"), project_dir.join("config.toml"))
        .context("link the nested candidate at the project file")?;
    let user_file = write_config(&base.join(".netsuke.toml"), "jobs = 4\n")?;

    let environment = case.scoped_environment(
        TestEnv::default()
            .with_var("HOME", case.home().as_os_str())
            .with_var("XDG_CONFIG_HOME", base.as_os_str()),
    );
    let found = run_with_project_key(&environment, &project_file, Some(&project_dir))
        .context("the refused nested candidate must not fail discovery")?;

    ensure!(
        found.user_scope == UserScopeTrace::Fallback,
        "the nested alias of the project file must not be adopted, got {:?}",
        found.user_scope
    );
    ensure!(
        layer_paths(&found)? == vec![canonical(&user_file)?],
        "the distinct user file in the refused base must supply the chain alone, got {:?}",
        layer_paths(&found)?
    );
    ensure!(
        found.is_trusted_root,
        "a candidate provably distinct from the project file keeps operator authority"
    );
    Ok(())
}

/// A user candidate whose identity cannot be established is not adopted.
///
/// Trust is granted only to a candidate that provably names another file. The
/// project key resolves under the real normalizer, so this case isolates the
/// candidate: it cannot be ruled out as the project file, and adopting it would
/// grant operator authority on a guess. Refusing it is still decisive, so the
/// dependency scan supplies the base chain instead.
#[rstest]
fn unresolvable_user_candidate_is_not_adopted() -> Result<()> {
    let case = Case::new()?;
    let user_file = write_config(&case.home().join(".netsuke.toml"), "jobs = 4\n")?;
    let project_dir = case.base("elsewhere")?;
    let project_file = write_config(&project_dir.join(".netsuke.toml"), "jobs = 9\n")?;
    let project_key = ProjectKey::classify(&FsPathNormalizer, &project_file);

    let found = run_with_normalizer(
        &case.environment(HomeArrangement::EmptyXdgConfigHome),
        &FailingPathNormalizer,
        Some(&project_key),
        None,
    )
    .context("an unresolvable candidate must not fail discovery")?;

    ensure!(
        found.user_scope == UserScopeTrace::Fallback,
        "a candidate whose identity cannot be established must not be adopted, got {:?}",
        found.user_scope
    );
    ensure!(
        !found.is_trusted_root,
        "a chain whose candidate cannot be ruled out as the project file must not report a trusted root"
    );
    ensure!(
        user_file != project_file,
        "the fixture must keep the two paths distinct"
    );
    Ok(())
}

/// A user file distinct from the project file is adopted with its authority.
///
/// Without this case, an implementation that refused every user candidate would
/// satisfy the rejection tests above while silently downgrading ordinary user
/// configuration. The adopted chain must also name the user file itself: a pass
/// that reported adoption while loading something else would otherwise pass.
#[rstest]
fn user_file_distinct_from_the_project_file_is_adopted() -> Result<()> {
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
        matches!(found.user_scope, UserScopeTrace::Adopted { .. }),
        "a user file that is not the project file should be adopted, got {:?}",
        found.user_scope
    );
    ensure!(
        found.is_trusted_root,
        "a user candidate that is not the project file must keep operator authority"
    );
    ensure!(
        layer_paths(&found)?.contains(&canonical(&user_file)?),
        "the adopted chain must name the winning user file, got {:?}",
        layer_paths(&found)?
    );
    ensure!(
        user_file != project_file,
        "the fixture must keep the two paths distinct"
    );
    Ok(())
}
