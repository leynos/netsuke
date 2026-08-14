//! Integration tests for the registered `expanduser` `MiniJinja` filter.
//!
//! The precedence ladders are covered exhaustively at their injected-reader
//! seam (`src/stdlib/path/home_tests.rs`); these cases prove the *registered
//! filter* — the template-visible boundary — behaves per contract. The
//! successful `~/...` cases run against a home configured through
//! `with_home_override`, so no test consults or mutates the process
//! environment, which the AGENTS.md testing mandate forbids; the `Ambient`
//! ladders behind the same registration closure are the seam tests' concern.

use anyhow::{Context, Result, anyhow, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::{Environment, context};
use netsuke::stdlib::{self, StdlibConfig};
use rstest::{fixture, rstest};

struct StdlibWorkspace {
    _temp: tempfile::TempDir,
    root: Utf8PathBuf,
}

#[fixture]
fn stdlib_workspace() -> Result<StdlibWorkspace> {
    let temp = tempfile::tempdir().context("create temp workspace")?;
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
        .map_err(|path| anyhow!("temp path should be UTF-8: {}", path.display()))?;
    Ok(StdlibWorkspace { _temp: temp, root })
}

fn stdlib_env(root: &Utf8Path) -> Result<Environment<'static>> {
    let workspace = Dir::open_ambient_dir(root, ambient_authority())
        .with_context(|| format!("open workspace {root}"))?;
    let config = StdlibConfig::new(workspace)?.with_workspace_root_path(root.to_path_buf())?;
    let mut env = Environment::new();
    stdlib::register_with_config(&mut env, config)?;
    Ok(env)
}

fn render(env: &Environment<'static>, template: &str) -> Result<String, minijinja::Error> {
    env.render_str(template, context! {})
}

/// A tilde path renders to the configured home through the registered filter.
///
/// This is the end-to-end success path at the template-visible boundary: the
/// home comes from `with_home_override`, so the ladder consults no ambient
/// environment, and the assertion covers the full route — template, filter
/// registration, `expanduser`, and home resolution.
#[rstest]
#[case::bare_tilde("{{ '~' | expanduser }}", "")]
#[case::tilde_subpath("{{ '~/notes/todo.txt' | expanduser }}", "/notes/todo.txt")]
fn a_tilde_path_expands_against_the_configured_home(
    stdlib_workspace: Result<StdlibWorkspace>,
    #[case] template: &str,
    #[case] suffix: &str,
) -> Result<()> {
    let workspace = stdlib_workspace?;
    let home = workspace.root.join("home/dweller");
    let dir = Dir::open_ambient_dir(&workspace.root, ambient_authority())
        .with_context(|| format!("open workspace {}", workspace.root))?;
    let config = StdlibConfig::new(dir)?
        .with_workspace_root_path(&workspace.root)?
        .with_home_override(Some(home.to_string()));
    let mut env = Environment::new();
    stdlib::register_with_config(&mut env, config)?;

    let rendered = render(&env, template).context("a tilde path should render")?;
    let expected = format!("{home}{suffix}");
    ensure!(
        rendered == expected,
        "expected {expected:?}, got {rendered:?}"
    );
    Ok(())
}

/// A path without a leading tilde passes through the filter unchanged.
///
/// This branch reads no environment at all, so it proves the registration —
/// name, argument plumbing, and return path — with no ambient dependency.
#[rstest]
fn a_non_tilde_path_passes_through_unchanged(
    stdlib_workspace: Result<StdlibWorkspace>,
) -> Result<()> {
    let workspace = stdlib_workspace?;
    let env = stdlib_env(&workspace.root)?;
    let rendered = render(&env, "{{ '/etc/hosts' | expanduser }}")
        .context("a non-tilde path should render")?;
    ensure!(
        rendered == "/etc/hosts",
        "expected passthrough, got {rendered:?}"
    );
    Ok(())
}

/// Named-user expansion is rejected at the filter boundary.
///
/// The rejection happens before any environment read, so the error is
/// deterministic on every host; asserting on the rendered error proves the
/// filter surfaces it to templates rather than swallowing it.
#[rstest]
fn named_user_forms_are_rejected_at_the_filter(
    stdlib_workspace: Result<StdlibWorkspace>,
) -> Result<()> {
    let workspace = stdlib_workspace?;
    let env = stdlib_env(&workspace.root)?;
    let error = render(&env, "{{ '~alice/notes' | expanduser }}")
        .err()
        .context("~alice should be rejected")?;
    ensure!(
        error.to_string().contains("expansion is unsupported"),
        "the error should state the unsupported expansion, got {error}"
    );
    Ok(())
}
