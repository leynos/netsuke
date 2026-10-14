//! Exercise derivation through isolated repository fixtures.
use super::*;
use anyhow::ensure;
use camino::Utf8Path;
use rstest::rstest;

/// Build a compact survey containing every required prose witness.
fn document() -> String {
    let mut text = String::from(concat!(
        "## 6. Cross-cutting contract\n### 6.1. Purity classes\n",
        "Of the fifty-seven helpers proposed here, fifty-two are pure, four are filesystem-observing, and one is environment-observing.\n",
        "## 7. Candidate matrix\n### 7.1. Core filters\n",
        "| Name | Alias | Decision | Resolution |\n|---|---|---|---|\n",
        "| `hash` | h | Accept as `text_hash` | §8.9 |\n",
        "| `quote` | q | Reject | none |\n",
        "| `win_splitdrive` | w | Reject | none |\n",
        "| `basename` | b | Reject | §8.6 |\n",
        "| `dirname` | d | Reject | §8.6 |\n",
        "| `fileglob` | f | Reject | §8.7 |\n",
        "| `denied` | d | Defer | none |\n",
        "### 7.8. Totals\n| Measure | Count |\n|---|---|\n"
    ));
    let rows = [
        "Surveyed entries accepted",
        "Surveyed entries deferred",
        "Surveyed entries rejected because",
        "Surveyed entries rejected as a redundant alias",
        "Surveyed entries rejected on principle",
        "New Netsuke filters introduced",
        "New Netsuke tests introduced",
        "Existing Netsuke helpers gaining",
    ]
    .into_iter()
    .map(|label| format!("| {label} | 1 |\n"))
    .collect::<Vec<_>>()
    .concat();
    text.push_str(&rows);
    text.push_str(concat!(
        "## 8. Accepted capabilities\n### 8.6. Paths\n",
        "basename dirname splitdrive\n### 8.7. Filesystem\nglob\n",
        "### 8.9. Text\ntext_hash shell_quote\n"
    ));
    text
}

#[test]
fn missing_survey_file_reports_repository_relative_path() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let root = Utf8Path::from_path(temp.path()).context("UTF-8 temporary path")?;
    let repo = Repo::fixture(root)?;
    let error = derive(&repo).err().context("missing survey should fail")?;
    ensure!(
        format!("{error:#}").contains(&format!("read {RFC_0006}")),
        "missing file error lost source context: {error:#}"
    );
    Ok(())
}

#[rstest]
#[case::missing_matrix("## 7. Candidate matrix", "## 7. Removed", "RFC 0006 has no section 7")]
#[case::missing_rename(
    "`quote`",
    "`different`",
    "rename exception quote has no section 7 row"
)]
#[case::missing_contract("text_hash shell_quote", "different", "never named as whole tokens")]
fn derivation_propagates_specific_parser_failures(
    #[case] old: &str,
    #[case] new: &str,
    #[case] diagnostic: &str,
) -> Result<()> {
    let temp = tempfile::tempdir()?;
    let root = Utf8Path::from_path(temp.path()).context("UTF-8 temporary path")?;
    let repo = Repo::fixture(root)?;
    repo.dir.create_dir_all("docs/rfcs")?;
    repo.dir.write(RFC_0006, document().replace(old, new))?;
    let error = derive(&repo).err().context("invalid fixture should fail")?;
    ensure!(
        format!("{error:#}").contains(diagnostic),
        "expected {diagnostic:?}, got {error:#}"
    );
    Ok(())
}

#[test]
fn derivation_complements_denied_names_and_groups_owned_sections() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let root = Utf8Path::from_path(temp.path()).context("UTF-8 temporary path")?;
    let repo = Repo::fixture(root)?;
    repo.dir.create_dir_all("docs/rfcs")?;
    repo.dir.write(RFC_0006, document())?;
    let survey = derive(&repo)?;
    ensure!(
        survey.denied.contains("denied"),
        "deferred candidate must be denied"
    );
    ensure!(
        survey.denied.contains("quote"),
        "rejected spelling must be denied"
    );
    ensure!(
        !survey.denied.contains("basename"),
        "optioned helper cannot be denied"
    );
    ensure!(
        survey.sections.get("8.6").context("path section")?.len() == 3,
        "path section must have three helpers"
    );
    ensure!(
        survey.new_by_namespace(Namespace::Filter) == 3,
        "expected three new filters"
    );
    ensure!(
        survey.new_by_namespace(Namespace::Function) == 0,
        "optioned glob must not count as new"
    );
    // The fixture is parse-valid, but deliberately does not impersonate full survey counts.
    let error = derive_and_check(&repo)
        .err()
        .context("inconsistent totals must fail")?;
    ensure!(
        error.to_string().contains("derived 5 reject rows"),
        "{error:#}"
    );
    Ok(())
}
