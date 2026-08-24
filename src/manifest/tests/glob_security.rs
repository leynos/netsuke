//! Security regression coverage for filesystem paths exposed by Jinja `glob()`.

use super::super::from_str;
use crate::snapshot_test_support::snapshot_settings;
use anyhow::{Result, ensure};
use insta::assert_snapshot;
use rstest::rstest;
use tempfile::tempdir;
use test_support::{display_error_chain, fs as test_fs, manifest::manifest_yaml};

fn glob_manifest(pattern: &str) -> String {
    manifest_yaml(&format!(
        concat!(
            "targets:\n",
            "  - foreach: glob({pattern:?})\n",
            "    name: output-{{{{ index }}}}.txt\n",
            "    command: echo {{{{ item }}}}\n"
        ),
        pattern = pattern
    ))
}

#[cfg(unix)]
#[rstest]
fn jinja_glob_rejects_a_command_injecting_filename() -> Result<()> {
    let temp = tempdir()?;
    test_fs::write(
        temp.path().join("a.txt; touch PWNED; #.txt"),
        "attacker-controlled filename",
    )?;
    let pattern = format!("{}/*.txt", temp.path().display());

    let error = from_str(&glob_manifest(&pattern))
        .expect_err("shell syntax in a matched filename must stop manifest loading");
    let diagnostic = format!("{error:#}");
    ensure!(
        diagnostic.contains("characters that require shell quoting"),
        "unexpected diagnostic: {diagnostic}"
    );
    let rendered =
        display_error_chain(error.as_ref()).replace(&temp.path().display().to_string(), "[TEMP]");
    snapshot_settings("manifest").bind(|| {
        assert_snapshot!("jinja_glob_unsafe_path", rendered);
    });
    Ok(())
}

#[rstest]
fn jinja_glob_preserves_a_shell_inert_filename() -> Result<()> {
    let temp = tempdir()?;
    test_fs::write(temp.path().join("safe-file_1.txt"), "ordinary filename")?;
    let pattern = format!("{}/*.txt", temp.path().display());

    let manifest = from_str(&glob_manifest(&pattern))?;
    ensure!(
        manifest.targets.len() == 1,
        "the ordinary matched filename should still expand once"
    );
    Ok(())
}
