//! Unit tests for atomic dyndep sidecar materialization.

use super::*;
use crate::ninja_gen::GeneratedDyndep;
use anyhow::{Result, ensure};
use camino::Utf8PathBuf;

fn sidecar(name: &str, content: &str) -> GeneratedDyndep {
    GeneratedDyndep::fixture(Utf8PathBuf::from(name), content.to_owned())
}

fn temp_dir(temp: &tempfile::TempDir) -> Result<Dir> {
    let path = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
        .map_err(|path| anyhow!("temporary directory is not UTF-8: {}", path.display()))?;
    Dir::open_ambient_dir(path, cap_std::ambient_authority()).map_err(Into::into)
}

#[test]
fn materializes_nested_sidecar_and_reuses_it() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let dir = temp_dir(&temp)?;
    let dyndep = sidecar(".netsuke/dyndep/abc.dd", "ninja_dyndep_version = 1\n");

    materialize_dyndep_files(&dir, &[dyndep])?;
    ensure_matching(&dir, ".netsuke/dyndep/abc.dd", "ninja_dyndep_version = 1\n")?;

    // Second run reuses the existing sidecar without error.
    materialize_dyndep_files(
        &dir,
        &[sidecar(
            ".netsuke/dyndep/abc.dd",
            "ninja_dyndep_version = 1\n",
        )],
    )?;
    Ok(())
}

#[test]
fn empty_sidecar_list_does_not_create_dyndep_directory() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let dir = temp_dir(&temp)?;

    materialize_dyndep_files(&dir, &[])?;

    ensure!(
        dir.open(DYNDEP_DIR).is_err(),
        "empty sidecar list must not create {DYNDEP_DIR}"
    );
    Ok(())
}

#[test]
fn corrupt_existing_sidecar_is_reported() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let dir = temp_dir(&temp)?;
    dir.create_dir_all(DYNDEP_DIR)?;
    dir.write(".netsuke/dyndep/bad.dd", "corrupt")?;

    let result = materialize_dyndep_files(&dir, &[sidecar(".netsuke/dyndep/bad.dd", "expected")]);
    ensure!(result.is_err(), "corrupt sidecar must be reported");
    Ok(())
}

#[test]
fn oversized_existing_sidecar_is_rejected() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let dir = temp_dir(&temp)?;
    let rel = ".netsuke/dyndep/oversized.dd";
    dir.create_dir_all(DYNDEP_DIR)?;
    let oversized_size = usize::try_from(MAX_VERIFIED_DYNDEP_SIZE + 1)?;
    dir.write(rel, vec![b'x'; oversized_size])?;

    let error = materialize_dyndep_files(&dir, &[sidecar(rel, "expected")])
        .expect_err("an oversized existing sidecar must be rejected");
    let expected = localization::message(keys::RUNNER_IO_DYNDEP_TOO_LARGE)
        .with_arg("path", rel)
        .with_arg("limit", MAX_VERIFIED_DYNDEP_SIZE)
        .to_string();
    ensure!(
        format!("{error:#}").contains(&expected),
        "expected localized oversized-sidecar error, got: {error:#}"
    );
    Ok(())
}

#[test]
fn no_temp_files_left_behind() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let dir = temp_dir(&temp)?;
    materialize_dyndep_files(&dir, &[sidecar(".netsuke/dyndep/x.dd", "content")])?;
    ensure_no_temp_files(&dir)?;
    Ok(())
}

#[test]
fn failed_atomic_write_removes_temp_file() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let dir = temp_dir(&temp)?;
    let rel = Utf8Path::new(".netsuke/dyndep/destination.dd");
    dir.create_dir_all(rel)?;

    let result = write_atomic(&dir, rel, "content");

    ensure!(result.is_err(), "rename over a directory must fail");
    ensure_no_temp_files(&dir)?;
    Ok(())
}

#[test]
fn stale_temp_file_does_not_block_materialization() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let dir = temp_dir(&temp)?;
    let rel = Utf8Path::new(".netsuke/dyndep/stale.dd");
    let content = "ninja_dyndep_version = 1\n";
    dir.create_dir_all(DYNDEP_DIR)?;
    dir.write(unique_temp_name(rel), "stale temporary content")?;

    materialize_dyndep_files(&dir, &[sidecar(rel.as_str(), content)])?;

    ensure_matching(&dir, rel.as_str(), content)
}

#[test]
fn separate_temp_names_for_same_sidecar_differ() {
    let rel = Utf8Path::new(".netsuke/dyndep/names.dd");
    let first = unique_temp_name(rel);
    let second = unique_temp_name(rel);

    assert_ne!(first, second, "temporary names must differ per attempt");
    assert_eq!(first.parent(), rel.parent());
    assert_eq!(second.parent(), rel.parent());
}

#[test]
fn matching_final_sidecar_succeeds_with_another_temp_file() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let dir = temp_dir(&temp)?;
    let rel = Utf8Path::new(".netsuke/dyndep/matching.dd");
    let content = "ninja_dyndep_version = 1\n";
    dir.create_dir_all(DYNDEP_DIR)?;
    dir.write(rel, content)?;
    dir.write(unique_temp_name(rel), "concurrent temporary content")?;

    write_atomic(&dir, rel, content)?;

    ensure_matching(&dir, rel.as_str(), content)
}

fn ensure_matching(dir: &Dir, path: &str, expected: &str) -> Result<()> {
    anyhow::ensure!(
        dir.read_to_string(path)? == expected,
        "sidecar content does not match"
    );
    Ok(())
}

fn ensure_no_temp_files(dir: &Dir) -> Result<()> {
    let names = dir
        .read_dir(DYNDEP_DIR)?
        .map(|entry| entry.and_then(|item| item.file_name()))
        .collect::<std::io::Result<Vec<_>>>()?;
    ensure!(
        names.iter().all(|name| {
            Utf8Path::new(name)
                .extension()
                .is_none_or(|extension| !extension.eq_ignore_ascii_case("tmp"))
        }),
        "temporary files left behind: {names:?}"
    );
    Ok(())
}
