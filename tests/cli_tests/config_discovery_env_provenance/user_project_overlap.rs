//! Regression for a user location that is also the project configuration.
//!
//! Two layouts make one file both a user candidate and the project
//! configuration: running Netsuke from `$HOME` puts `$HOME/.netsuke.toml` in
//! both roles, and pointing `XDG_CONFIG_HOME` at the project directory makes
//! the base dotfile do the same. Both cases drive the merge in a child process,
//! because the discovery pass would otherwise skip the system layers and
//! silently drop the restrictions they set. They live apart from the provenance
//! cases so the parent module stays within the file-size limit.

use super::*;
use rstest::rstest;

/// A home dotfile that is also the project file keeps the system block.
///
/// The user pass must not adopt the candidate, because the system layers would
/// then be skipped and a restriction that exists only in the system file would
/// silently stop applying. The project value must still take effect, so the
/// case cannot pass by ignoring the file.
#[cfg(unix)]
#[rstest]
fn home_dotfile_that_is_the_project_file_keeps_the_system_block() -> Result<()> {
    let home = tempdir().context("create home directory")?;
    let system = tempdir().context("create system directory")?;
    write_config(
        system.path().join("netsuke/config.toml"),
        "env_block_var = [\"SAFE\"]\n",
    )?;
    write_config(home.path().join(".netsuke.toml"), "jobs = 2\n")?;
    let environment = environment_with_system_scope(home.path(), system.path(), &[]);
    let directory = home.path().to_str().context("home path is UTF-8")?;

    let cli = merge_in_child(
        &["netsuke", "--directory", directory],
        home.path(),
        &environment,
    )?;

    assert_env_name_blocked(&cli, "SAFE")?;
    ensure!(
        cli.jobs == Some(2),
        "the project file must still supply its own values, got {:?}",
        cli.jobs
    );
    Ok(())
}

/// An `XDG_CONFIG_HOME` base dotfile that is also the project file keeps the
/// system block.
///
/// The dependency lists the `XDG_CONFIG_HOME` base candidates before every
/// system candidate, so the fallback scan would return this project file first
/// and the system layers would never load. The scan must skip the candidate the
/// user pass already refused and keep walking to the system scope. The project
/// value must still take effect, so the case cannot pass by ignoring the file.
#[cfg(unix)]
#[rstest]
fn xdg_base_dotfile_that_is_the_project_file_keeps_the_system_block() -> Result<()> {
    let project = tempdir().context("create project directory")?;
    let home = tempdir().context("create home directory")?;
    let system = tempdir().context("create system directory")?;
    write_config(
        system.path().join("netsuke/config.toml"),
        "env_block_var = [\"SAFE\"]\n",
    )?;
    write_config(project.path().join(".netsuke.toml"), "jobs = 3\n")?;
    let environment = environment_with_xdg_config_home(home.path(), project.path(), system.path());
    let directory = project.path().to_str().context("project path is UTF-8")?;

    let cli = merge_in_child(
        &["netsuke", "--directory", directory],
        project.path(),
        &environment,
    )?;

    assert_env_name_blocked(&cli, "SAFE")?;
    ensure!(
        cli.jobs == Some(3),
        "the project file must still supply its own values, got {:?}",
        cli.jobs
    );
    Ok(())
}

/// Build an environment whose `XDG_CONFIG_HOME` is the project directory.
///
/// The shared helper points that variable at the home directory, which is the
/// layout the ordinary user-scope cases need. This one deliberately makes the
/// base dotfile the project configuration instead.
fn environment_with_xdg_config_home(
    home: &Path,
    project: &Path,
    system: &Path,
) -> Vec<(OsString, OsString)> {
    vec![
        (OsString::from("HOME"), home.as_os_str().to_owned()),
        (
            OsString::from("XDG_CONFIG_HOME"),
            project.as_os_str().to_owned(),
        ),
        (
            OsString::from("XDG_CONFIG_DIRS"),
            system.as_os_str().to_owned(),
        ),
    ]
}
