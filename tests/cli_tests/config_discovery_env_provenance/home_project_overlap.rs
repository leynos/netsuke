//! Regression for a user location that is also the project configuration.
//!
//! Running Netsuke from `$HOME` makes `$HOME/.netsuke.toml` both a user
//! candidate and the project configuration. This case drives the merge in a
//! child process, because the user pass would otherwise skip the system layers
//! and silently drop the restrictions they set. It lives apart from the
//! provenance cases so the parent module stays within the file-size limit.

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
