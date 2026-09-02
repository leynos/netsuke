//! Regression tests proving verbose tracing never emits rendered secrets.

use super::support::open_workspace;
use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs_utf8::Dir};
use rstest::rstest;
use tempfile::tempdir;
use test_support::netsuke::run_netsuke_in_with_env;

/// Distinctive sentinel injected through `env('CI_SECRET')` in the fixture.
const SENTINEL: &str = "CI-SECRET-7f3a9c2b5e1d4f60";

#[rstest]
fn verbose_generate_never_emits_rendered_secret() -> Result<()> {
    let temp = tempdir().context("create temporary workspace")?;
    let workspace = open_workspace(&temp)?;
    let repository = Dir::open_ambient_dir(env!("CARGO_MANIFEST_DIR"), ambient_authority())
        .context("open repository root")?;
    repository
        .copy(
            "tests/data/env_secret_sites.yml",
            &workspace,
            "Netsukefile",
        )
        .context("copy secret fixture into workspace")?;

    let run = run_netsuke_in_with_env(
        temp.path(),
        &["--verbose", "generate", "--output", "build.ninja"],
        &[("CI_SECRET", SENTINEL)],
    )?;

    ensure!(
        run.success,
        "verbose generate should succeed: {}",
        run.stderr
    );
    ensure!(
        !run.stdout.contains(SENTINEL),
        "rendered secret must not appear on stdout"
    );
    ensure!(
        !run.stderr.contains(SENTINEL),
        "rendered secret must not appear on stderr"
    );
    let generated =
        workspace.read_to_string("build.ninja").context("read generated Ninja file")?;
    ensure!(
        generated.contains(SENTINEL),
        "env('CI_SECRET') must still resolve in generated output"
    );
    Ok(())
}
