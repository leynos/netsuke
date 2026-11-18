//! Unit tests for same-directory temporary files and atomic sidecar writes.

use super::*;
#[test]
fn no_temp_files_left_behind() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let dir = temporary_dir(&temp)?;
    materialize_dyndep_files(&dir, &[sidecar(".netsuke/dyndep/x.dd", "content")])?;
    ensure_no_temp_files(&dir)?;
    Ok(())
}

#[test]
fn failed_atomic_write_removes_temp_file() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let dir = temporary_dir(&temp)?;
    dir.create_dir_all(DYNDEP_DIR)?;
    let dyndep = dir.open_dir(DYNDEP_DIR)?;
    let name = Utf8Path::new("destination.dd");
    dyndep.create_dir(name)?;

    let result = write_atomic(&dyndep, name, "content");

    ensure!(result.is_err(), "rename over a directory must fail");
    ensure_no_temp_files(&dir)?;
    Ok(())
}

#[test]
fn stale_temp_file_does_not_block_materialization() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let dir = temporary_dir(&temp)?;
    let rel = ".netsuke/dyndep/stale.dd";
    let content = "ninja_dyndep_version = 1\n";
    dir.create_dir_all(DYNDEP_DIR)?;
    let stale = TempNameSource::new("stale".to_owned()).next_name(Utf8Path::new("stale.dd"));
    dir.write(
        Utf8Path::new(DYNDEP_DIR).join(&stale),
        "stale temporary content",
    )?;

    materialize_dyndep_files(&dir, &[sidecar(rel, content)])?;

    ensure_matching(&dir, rel, content)
}

#[test]
fn separate_temp_names_for_same_sidecar_differ() {
    let name = Utf8Path::new("names.dd");
    let mut names = TempNameSource::new("fixture".to_owned());
    let first = names.next_name(name);
    let second = names.next_name(name);

    assert_ne!(first, second, "temporary names must differ per attempt");
    assert_eq!(first.parent(), name.parent());
    assert_eq!(second.parent(), name.parent());
}

#[test]
fn temporary_name_collision_retries_are_bounded() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let dir = temporary_dir(&temp)?;
    let name = Utf8Path::new("collisions.dd");
    dir.create_dir_all(DYNDEP_DIR)?;
    let dyndep = dir.open_dir(DYNDEP_DIR)?;
    let mut occupied_names = TempNameSource::new("collisions".to_owned());
    for _ in 0..MAX_TEMP_FILE_ATTEMPTS {
        dyndep.write(occupied_names.next_name(name), "occupied")?;
    }
    let mut attempted_names = TempNameSource::new("collisions".to_owned());

    let (result, snapshot) =
        recorded(|| create_unique_temp_file_with_source(&dyndep, name, &mut attempted_names));
    let error = result
        .err()
        .context("all occupied temporary names must exhaust the retry bound")?;
    let expected = localization::message(keys::RUNNER_IO_DYNDEP_TEMP_COLLISIONS)
        .with_arg("path", entry_display_path(name.as_str()))
        .to_string();
    ensure!(
        format!("{error:#}").contains(&expected),
        "expected localized collision error, got: {error:#}"
    );
    ensure!(
        temporary_file_retry_count(&snapshot, "retry") == Some(MAX_TEMP_FILE_ATTEMPTS as u64),
        "every collision must record a retry"
    );
    ensure!(
        temporary_file_retry_count(&snapshot, "exhausted") == Some(1),
        "retry exhaustion must record its fixed outcome"
    );
    Ok(())
}

#[test]
fn temporary_name_collision_retry_is_observed_before_success() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let dir = temporary_dir(&temp)?;
    let name = Utf8Path::new("retry.dd");
    dir.create_dir_all(DYNDEP_DIR)?;
    let dyndep = dir.open_dir(DYNDEP_DIR)?;
    let mut occupied_names = TempNameSource::new("retry".to_owned());
    dyndep.write(occupied_names.next_name(name), "occupied")?;
    let mut attempted_names = TempNameSource::new("retry".to_owned());

    let (result, snapshot) =
        recorded(|| create_unique_temp_file_with_source(&dyndep, name, &mut attempted_names));
    let (temp_path, file) = result?;
    drop(file);
    dyndep.remove_file(temp_path)?;

    ensure!(
        temporary_file_retry_count(&snapshot, "retry") == Some(1),
        "a recovered collision must record one retry"
    );
    ensure!(
        temporary_file_retry_count(&snapshot, "exhausted").is_none(),
        "a successful retry must not record exhaustion"
    );
    Ok(())
}

#[test]
fn matching_final_sidecar_succeeds_with_another_temp_file() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let dir = temporary_dir(&temp)?;
    let rel = ".netsuke/dyndep/matching.dd";
    let name = Utf8Path::new("matching.dd");
    let content = "ninja_dyndep_version = 1\n";
    dir.create_dir_all(DYNDEP_DIR)?;
    let dyndep = dir.open_dir(DYNDEP_DIR)?;
    dyndep.write(name, content)?;
    let other_temp = TempNameSource::new("concurrent".to_owned()).next_name(name);
    dyndep.write(other_temp, "concurrent temporary content")?;

    write_atomic(&dyndep, name, content)?;

    ensure_matching(&dir, rel, content)
}
