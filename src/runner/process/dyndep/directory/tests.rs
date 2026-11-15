//! Focused tests for verified dyndep directory establishment.

use super::super::super::tests::temporary_dir;
use super::*;
use anyhow::{Result, ensure};
use cap_std::fs_utf8::Dir;
use rstest::{fixture, rstest};

/// Stage an ambient workspace with a capability handle for one directory test.
#[fixture]
fn workspace() -> Result<(tempfile::TempDir, Dir)> {
    let temp = tempfile::tempdir()?;
    let dir = temporary_dir(&temp)?;
    Ok((temp, dir))
}

/// Assert that `result` refused the alias at `path` with the localized message.
fn assert_alias_refusal<T>(result: Result<T>, path: &str) -> Result<()> {
    let Err(error) = result else {
        anyhow::bail!("a generated-state alias must be refused");
    };
    let expected = localization::message(keys::RUNNER_IO_DYNDEP_ALIAS)
        .with_arg("path", path)
        .to_string();
    ensure!(
        format!("{error:#}").contains(&expected),
        "the refusal must name the aliased component: {error:#}"
    );
    Ok(())
}

#[rstest]
fn open_or_create_creates_the_dyndep_namespace(
    workspace: Result<(tempfile::TempDir, Dir)>,
) -> Result<()> {
    let (_temp, dir) = workspace?;

    let dyndep = DyndepDirectory::open_or_create(&dir)?;

    ensure!(
        dyndep.dir().dir_metadata()?.is_dir(),
        "the returned handle must be a real directory"
    );
    ensure!(
        dir.open(DYNDEP_DIR).is_ok(),
        "the namespace must exist beneath the workspace"
    );
    Ok(())
}

#[rstest]
fn open_or_create_reuses_an_existing_namespace(
    workspace: Result<(tempfile::TempDir, Dir)>,
) -> Result<()> {
    let (_temp, dir) = workspace?;
    dir.create_dir_all(DYNDEP_DIR)?;
    dir.write(".netsuke/dyndep/kept.dd", "kept")?;

    let dyndep = DyndepDirectory::open_or_create(&dir)?;

    ensure!(
        dyndep.dir().read_to_string("kept.dd")? == "kept",
        "an existing namespace must be reused without disturbing its contents"
    );
    Ok(())
}

#[rstest]
fn open_existing_reports_an_absent_namespace(
    workspace: Result<(tempfile::TempDir, Dir)>,
) -> Result<()> {
    let (_temp, dir) = workspace?;
    ensure!(
        DyndepDirectory::open_existing(&dir)?.is_none(),
        "a workspace without .netsuke must report an absent namespace"
    );

    dir.create_dir(".netsuke")?;
    ensure!(
        DyndepDirectory::open_existing(&dir)?.is_none(),
        "a .netsuke directory without dyndep must report an absent namespace"
    );
    Ok(())
}

#[rstest]
fn open_or_create_refuses_a_netsuke_regular_file(
    workspace: Result<(tempfile::TempDir, Dir)>,
) -> Result<()> {
    let (_temp, dir) = workspace?;
    dir.write(".netsuke", "not a directory")?;

    assert_alias_refusal(DyndepDirectory::open_or_create(&dir), ".netsuke")
}

#[cfg(unix)]
#[rstest]
fn open_or_create_refuses_a_netsuke_directory_symlink(
    workspace: Result<(tempfile::TempDir, Dir)>,
) -> Result<()> {
    let (temp, dir) = workspace?;
    let victim = temp.path().join("victim");
    test_support::fs::create_dir(&victim)?;
    test_support::fs::symlink("victim", temp.path().join(".netsuke"))?;

    assert_alias_refusal(DyndepDirectory::open_or_create(&dir), ".netsuke")?;
    ensure!(
        !test_support::fs::exists(victim.join("dyndep")),
        "creation must not follow the .netsuke alias"
    );
    Ok(())
}

#[cfg(unix)]
#[rstest]
fn open_existing_refuses_a_dyndep_directory_symlink(
    workspace: Result<(tempfile::TempDir, Dir)>,
) -> Result<()> {
    let (temp, dir) = workspace?;
    let victim = temp.path().join("victim");
    test_support::fs::create_dir(&victim)?;
    test_support::fs::create_dir(temp.path().join(".netsuke"))?;
    test_support::fs::symlink("../victim", temp.path().join(".netsuke/dyndep"))?;

    assert_alias_refusal(DyndepDirectory::open_existing(&dir), DYNDEP_DIR)
}

#[cfg(unix)]
#[rstest]
fn open_existing_refuses_a_dangling_dyndep_symlink(
    workspace: Result<(tempfile::TempDir, Dir)>,
) -> Result<()> {
    let (temp, dir) = workspace?;
    test_support::fs::create_dir(temp.path().join(".netsuke"))?;
    test_support::fs::symlink("absent", temp.path().join(".netsuke/dyndep"))?;

    // A dangling alias is still an alias: `open_existing` must refuse it rather
    // than translate the failed open into an absent namespace.
    assert_alias_refusal(DyndepDirectory::open_existing(&dir), DYNDEP_DIR)
}
