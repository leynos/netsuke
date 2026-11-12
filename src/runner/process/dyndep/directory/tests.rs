//! Focused tests for verified dyndep directory establishment.

use super::super::super::tests::temporary_dir;
use super::*;
use anyhow::{Result, ensure};
use cap_std::fs_utf8::Dir;
use rstest::rstest;

fn workspace() -> Result<(tempfile::TempDir, Dir)> {
    let temp = tempfile::tempdir()?;
    let dir = temporary_dir(&temp)?;
    Ok((temp, dir))
}

#[rstest]
fn open_or_create_creates_the_dyndep_namespace() -> Result<()> {
    let (_temp, dir) = workspace()?;

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
fn open_or_create_reuses_an_existing_namespace() -> Result<()> {
    let (_temp, dir) = workspace()?;
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
fn open_existing_reports_an_absent_namespace() -> Result<()> {
    let (_temp, dir) = workspace()?;

    ensure!(
        DyndepDirectory::open_existing(&dir)?.is_none(),
        "a workspace without .netsuke must report an absent namespace"
    );
    Ok(())
}

#[rstest]
fn open_existing_reports_an_absent_dyndep_directory() -> Result<()> {
    let (_temp, dir) = workspace()?;
    dir.create_dir(".netsuke")?;

    ensure!(
        DyndepDirectory::open_existing(&dir)?.is_none(),
        "a .netsuke directory without dyndep must report an absent namespace"
    );
    Ok(())
}

#[rstest]
fn open_or_create_refuses_a_netsuke_regular_file() -> Result<()> {
    let (_temp, dir) = workspace()?;
    dir.write(".netsuke", "not a directory")?;

    let Err(error) = DyndepDirectory::open_or_create(&dir) else {
        anyhow::bail!("a .netsuke regular file must be refused");
    };
    let expected = localization::message(keys::RUNNER_IO_DYNDEP_ALIAS)
        .with_arg("path", ".netsuke")
        .to_string();
    ensure!(
        format!("{error:#}").contains(&expected),
        "the refusal must name the non-directory component: {error:#}"
    );
    Ok(())
}

#[cfg(unix)]
#[rstest]
fn open_or_create_refuses_a_netsuke_directory_symlink() -> Result<()> {
    let (temp, dir) = workspace()?;
    let victim = temp.path().join("victim");
    test_support::fs::create_dir(&victim)?;
    test_support::fs::symlink("victim", temp.path().join(".netsuke"))?;

    let Err(error) = DyndepDirectory::open_or_create(&dir) else {
        anyhow::bail!("a .netsuke symlink must be refused");
    };
    let expected = localization::message(keys::RUNNER_IO_DYNDEP_ALIAS)
        .with_arg("path", ".netsuke")
        .to_string();
    ensure!(
        format!("{error:#}").contains(&expected),
        "the refusal must name the aliased component: {error:#}"
    );
    ensure!(
        !test_support::fs::exists(victim.join("dyndep")),
        "creation must not follow the .netsuke alias"
    );
    Ok(())
}

#[cfg(unix)]
#[rstest]
fn open_existing_refuses_a_dyndep_directory_symlink() -> Result<()> {
    let (temp, dir) = workspace()?;
    let victim = temp.path().join("victim");
    test_support::fs::create_dir(&victim)?;
    test_support::fs::create_dir(temp.path().join(".netsuke"))?;
    test_support::fs::symlink("../victim", temp.path().join(".netsuke/dyndep"))?;

    let Err(error) = DyndepDirectory::open_existing(&dir) else {
        anyhow::bail!("a dyndep symlink must be refused");
    };
    let expected = localization::message(keys::RUNNER_IO_DYNDEP_ALIAS)
        .with_arg("path", DYNDEP_DIR)
        .to_string();
    ensure!(
        format!("{error:#}").contains(&expected),
        "the refusal must name the aliased component: {error:#}"
    );
    Ok(())
}

#[cfg(unix)]
#[rstest]
fn open_existing_refuses_a_dangling_dyndep_symlink() -> Result<()> {
    let (temp, dir) = workspace()?;
    test_support::fs::create_dir(temp.path().join(".netsuke"))?;
    test_support::fs::symlink("absent", temp.path().join(".netsuke/dyndep"))?;

    let Err(error) = DyndepDirectory::open_existing(&dir) else {
        anyhow::bail!("a dangling dyndep symlink must be refused");
    };
    let expected = localization::message(keys::RUNNER_IO_DYNDEP_ALIAS)
        .with_arg("path", DYNDEP_DIR)
        .to_string();
    ensure!(
        format!("{error:#}").contains(&expected),
        "a dangling alias must not be mistaken for an absent namespace: {error:#}"
    );
    Ok(())
}
