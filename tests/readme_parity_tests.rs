//! Exercise the manual README heading-parity checker with isolated fixtures.

#![cfg(unix)]

use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use pretty_assertions::assert_eq;
use rstest::rstest;
use std::process::{Command, Output};
use tempfile::{TempDir, tempdir};
use test_support::fs as test_fs;

const READMES: [&str; 7] = [
    "README.md",
    "README.de.md",
    "README.es.md",
    "README.fr.md",
    "README.ja.md",
    "README.pt-BR.md",
    "README.zh-CN.md",
];

struct ParityWorkspace {
    _directory: TempDir,
    root: Utf8PathBuf,
}

impl ParityWorkspace {
    /// Stage the published checker with seven independently editable READMEs.
    ///
    /// # Errors
    /// Return an error if the fixture directory or any file cannot be created.
    fn new(source: &str) -> Result<Self> {
        let directory = tempdir().context("create README parity fixture")?;
        let root = Utf8Path::from_path(directory.path())
            .context("temporary README workspace path must be UTF-8")?
            .to_path_buf();
        test_fs::create_dir_all(root.join("scripts"))?;
        test_fs::copy(
            Utf8Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/check-readme-parity.sh"),
            root.join("scripts/check-readme-parity.sh"),
        )?;
        let workspace = Self {
            _directory: directory,
            root,
        };
        for name in READMES {
            workspace.write(name, source)?;
        }
        Ok(workspace)
    }

    /// Replace one README to model a translated edition with a different shape.
    ///
    /// # Errors
    /// Return an error if the fixture README cannot be written.
    fn write(&self, name: &str, source: &str) -> Result<()> {
        test_fs::write(self.root.join(name), source).with_context(|| format!("write {name}"))
    }

    /// Execute the checker from the supplied directory without changing process state.
    ///
    /// # Errors
    /// Return an error if Bash cannot be started.
    fn run_from(&self, current_dir: &Utf8Path) -> Result<Output> {
        Command::new("bash")
            .arg(
                self.root
                    .join("scripts/check-readme-parity.sh")
                    .as_std_path(),
            )
            .current_dir(current_dir.as_std_path())
            .output()
            .context("execute README parity checker")
    }

    /// Execute the checker from its fixture project root.
    ///
    /// # Errors
    /// Return an error if Bash cannot be started.
    fn run(&self) -> Result<Output> {
        self.run_from(&self.root)
    }
}

/// Decode UTF-8 output so failures show the checker's actual report.
///
/// # Errors
/// Return an error if either stream is not valid UTF-8.
fn output_text(output: Output) -> Result<(Option<i32>, String, String)> {
    Ok((
        output.status.code(),
        String::from_utf8(output.stdout).context("checker stdout must be UTF-8")?,
        String::from_utf8(output.stderr).context("checker stderr must be UTF-8")?,
    ))
}

#[rstest]
fn matching_headings_use_levels_not_translated_words() -> Result<()> {
    let workspace = ParityWorkspace::new("# English\n## Contract\n### Detail\n")?;
    workspace.write("README.de.md", "# Deutsch\n## Vertrag\n### Detail\n")?;
    let (status, stdout, stderr) = output_text(workspace.run()?)?;
    assert_eq!(
        status,
        Some(0),
        "matching heading levels should pass: {stderr}"
    );
    assert_eq!(stdout.lines().count(), READMES.len());
    for name in READMES {
        ensure!(
            stdout.contains(&format!("{name} 3:#,##,###")),
            "missing or incorrect structure for {name}: {stdout}"
        );
    }
    assert_eq!(stderr, "");
    Ok(())
}

#[rstest]
#[case::missing_heading("# Title\n", "1:#")]
#[case::changed_level("# Title\n### Detail\n", "2:#,###")]
fn changed_heading_structure_fails(#[case] replacement: &str, #[case] actual: &str) -> Result<()> {
    let workspace = ParityWorkspace::new("# Title\n## Detail\n")?;
    workspace.write("README.de.md", replacement)?;
    let (status, stdout, stderr) = output_text(workspace.run()?)?;
    assert_eq!(status, Some(1), "changed heading structure should fail");
    ensure!(
        stdout.contains(&format!("README.de.md {actual}")),
        "{stdout}"
    );
    assert_eq!(stderr, "README heading structure differs: README.de.md\n");
    Ok(())
}

#[rstest]
fn reports_each_mismatched_translation() -> Result<()> {
    let workspace = ParityWorkspace::new("# Title\n## Detail\n")?;
    workspace.write("README.de.md", "# Title\n")?;
    workspace.write("README.fr.md", "# Title\n### Detail\n")?;
    let (status, _, stderr) = output_text(workspace.run()?)?;
    assert_eq!(status, Some(1), "both changed editions should fail");
    assert_eq!(
        stderr,
        concat!(
            "README heading structure differs: README.de.md\n",
            "README heading structure differs: README.fr.md\n"
        )
    );
    Ok(())
}

#[rstest]
#[case::backticks("# Title\n```yaml\n# Hidden\n```\n## Visible\n", "2:#,##")]
#[case::tildes("# Title\n~~~text\n# Hidden\n~~~\n## Visible\n", "2:#,##")]
#[case::short_fence("# Title\n``\n## Visible\n", "2:#,##")]
#[case::different_delimiter("# Title\n```\n# Hidden\n~~~\n# Hidden\n```\n## Visible\n", "2:#,##")]
#[case::longer_closer("# Title\n~~~\n# Hidden\n~~~~\n## Visible\n", "2:#,##")]
#[case::shorter_closer("# Title\n````\n# Hidden\n```\n# Hidden\n````\n## Visible\n", "2:#,##")]
#[case::trailing_text("# Title\n```\n``` note\n# Hidden\n```\n## Visible\n", "2:#,##")]
#[case::invalid_backtick_info("# Title\n```bad`info\n## Visible\n", "2:#,##")]
#[case::unclosed_fence("# Title\n```\n## Hidden\n", "1:#")]
fn fence_boundaries_control_heading_visibility(
    #[case] source: &str,
    #[case] structure: &str,
) -> Result<()> {
    let workspace = ParityWorkspace::new(source)?;
    let (status, stdout, stderr) = output_text(workspace.run()?)?;
    assert_eq!(
        status,
        Some(0),
        "identical structures should pass: {stderr}"
    );
    ensure!(
        stdout.contains(&format!("README.md {structure}\n")),
        "unexpected fence interpretation: {stdout}"
    );
    Ok(())
}

#[rstest]
fn heading_indentation_and_separators_match_markdown_boundaries() -> Result<()> {
    let workspace = ParityWorkspace::new(concat!(
        " # One\n  ## Two\n   ### Three\n",
        "    #### Indented code\n\t#### Tab-indented code\n",
        "####\tTabbed separator\n#####\n",
        "#word\n####### Too many hashes\n"
    ))?;
    let (status, stdout, stderr) = output_text(workspace.run()?)?;
    assert_eq!(
        status,
        Some(0),
        "identical structures should pass: {stderr}"
    );
    ensure!(
        stdout.contains("README.md 5:#,##,###,####,#####\n"),
        "{stdout}"
    );
    Ok(())
}

#[rstest]
fn crlf_and_lf_inputs_have_the_same_structure() -> Result<()> {
    let workspace =
        ParityWorkspace::new("# Title\r\n```yaml\r\n# Hidden\r\n```\r\n## Visible\r\n")?;
    workspace.write(
        "README.de.md",
        "# Titel\n```yaml\n# Verborgen\n```\n## Sichtbar\n",
    )?;
    let (status, stdout, stderr) = output_text(workspace.run()?)?;
    assert_eq!(
        status,
        Some(0),
        "line endings should not alter headings: {stderr}"
    );
    ensure!(stdout.contains("README.de.md 2:#,##\n"), "{stdout}");
    Ok(())
}

#[rstest]
fn finds_readmes_when_invoked_outside_repository() -> Result<()> {
    let workspace = ParityWorkspace::new("# Title\n")?;
    let other_directory = tempdir().context("create unrelated current directory")?;
    let other_path = Utf8Path::from_path(other_directory.path())
        .context("unrelated temporary directory path must be UTF-8")?;
    let (status, stdout, stderr) = output_text(workspace.run_from(other_path)?)?;
    assert_eq!(
        status,
        Some(0),
        "script-relative lookup should pass: {stderr}"
    );
    assert_eq!(stdout.lines().count(), READMES.len());
    Ok(())
}

#[rstest]
fn missing_readme_produces_a_nonzero_exit() -> Result<()> {
    let workspace = ParityWorkspace::new("# Title\n")?;
    test_fs::remove_file(workspace.root.join("README.zh-CN.md"))?;
    let (status, _, stderr) = output_text(workspace.run()?)?;
    ensure!(status != Some(0), "a missing edition must fail");
    ensure!(stderr.contains("README.zh-CN.md"), "{stderr}");
    Ok(())
}
