//! Security regression coverage for filesystem paths exposed by Jinja `glob()`.

use super::super::from_str;
#[cfg(unix)]
use crate::snapshot_test_support::snapshot_settings;
use anyhow::{Result, ensure};
#[cfg(unix)]
use insta::assert_snapshot;
use rstest::rstest;
use tempfile::tempdir;
#[cfg(unix)]
use test_support::display_error_chain;
use test_support::{fs as test_fs, manifest::manifest_yaml};

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

/// Render the manifest rejection diagnostic for one matched filename.
#[cfg(unix)]
fn rejection_diagnostic_for_filename(filename: &str) -> Result<String> {
    let temp = tempdir()?;
    test_fs::write(temp.path().join(filename), "attacker-controlled filename")?;
    let pattern = format!("{}/*.txt", temp.path().display());
    let error = from_str(&glob_manifest(&pattern))
        .expect_err("shell syntax in a matched filename must stop manifest loading");
    Ok(format!("{error:#}"))
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

#[cfg(unix)]
#[rstest]
#[case::whitespace("space file.txt")]
#[case::control_character("line\nbreak.txt")]
#[case::non_ascii("café.txt")]
#[case::single_quote("quote'file.txt")]
#[case::command_substitution("sub$(command).txt")]
#[case::backtick_substitution("sub`command`.txt")]
#[case::comment("comment#file.txt")]
#[case::redirection("redirect>file.txt")]
#[case::shell_punctuation("shell;pipe|ampersand&.txt")]
fn jinja_glob_rejects_every_representative_shell_requiring_filename(
    #[case] filename: &str,
) -> Result<()> {
    let diagnostic = rejection_diagnostic_for_filename(filename)?;
    ensure!(
        diagnostic.contains("characters that require shell quoting"),
        "unexpected diagnostic for {filename:?}: {diagnostic}"
    );
    Ok(())
}

#[cfg(unix)]
#[rstest]
fn jinja_glob_rejects_a_match_set_containing_safe_and_unsafe_paths() -> Result<()> {
    let temp = tempdir()?;
    test_fs::write(temp.path().join("safe-file_1.txt"), "ordinary filename")?;
    test_fs::write(
        temp.path().join("unsafe;file.txt"),
        "attacker-controlled filename",
    )?;
    let pattern = format!("{}/*.txt", temp.path().display());

    let error = from_str(&glob_manifest(&pattern))
        .expect_err("one unsafe match must stop the complete glob expansion");
    let diagnostic = format!("{error:#}");
    ensure!(
        diagnostic.contains("characters that require shell quoting"),
        "unexpected diagnostic: {diagnostic}"
    );
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
