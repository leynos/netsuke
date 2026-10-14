//! Direct clause-discharge, anti-vacuity, and whole-token diagnostics.

use super::*;
use rstest::rstest;

const FILE: &str = "docs/rfcs/0013-child.md";

/// Write an isolated parent or child contract document.
fn fixture(file: &str, text: &str) -> Result<(tempfile::TempDir, Repo)> {
    let temp = tempfile::tempdir()?;
    let root = camino::Utf8Path::from_path(temp.path()).context("UTF-8 fixture path")?;
    let repo = Repo::fixture(root)?;
    repo.dir.create_dir_all("docs/rfcs")?;
    repo.dir.write(file, text)?;
    Ok((temp, repo))
}

#[rstest]
#[case::missing("## 7. Survey", "has no section 6")]
#[case::no_clauses("## 6. Cross-cutting contract\nProse", "has no numbered subsections")]
fn parent_clause_structure_is_required(#[case] text: &str, #[case] expected: &str) -> Result<()> {
    let (_temp, repo) = fixture(RFC_0006, text)?;
    let error = clause_ids(&repo).expect_err("missing clauses must fail");
    ensure!(error.to_string().contains(expected), "{error}");
    Ok(())
}

#[test]
fn parent_clauses_ignore_fences_other_ids_and_repeated_ids() -> Result<()> {
    let text = concat!(
        "## 6. Cross-cutting contract\n### 6.1. Registry\n",
        "```markdown\n### 6.99. Example\n```\n",
        "### 6.1. Repeated\n### Other heading\n### 6.2. Namespace\n",
        "## 7. Survey\n### 6.3. Outside\n",
    );
    let (_temp, repo) = fixture(RFC_0006, text)?;
    {
        let actual = clause_ids(&repo)?;
        let expected_value = ["6.1", "6.2"];
        ensure!(
            actual == expected_value,
            "expected {expected_value:?}, found {actual:?}"
        );
    };
    Ok(())
}

#[rstest]
#[case::missing_heading("# Child", "no clause-discharge table")]
#[case::no_table("### Clause discharge\nProse", "contains no table")]
fn discharge_structure_is_required(#[case] text: &str, #[case] expected: &str) -> Result<()> {
    let (_temp, repo) = fixture(FILE, text)?;
    let error = discharged(&repo, FILE).expect_err("missing discharge must fail");
    ensure!(error.to_string().contains(expected), "{error}");
    ensure!(error.to_string().contains(FILE), "{error}");
    Ok(())
}

#[rstest]
#[case::bad_id("| `5.1` | `alpha` is pure. |", "not a section 6 clause", 4)]
#[case::empty("| `6.1` | `` |", "empty cell", 4)]
#[case::duplicate(
    "| `6.1` | `alpha` is pure. |\n| `6.1` | `alpha` is pure. |",
    "a second time",
    5
)]
#[case::missing_cell("| `6.1` |", "has no discharge column", 4)]
fn bad_discharge_rows_report_source_lines(
    #[case] rows: &str,
    #[case] expected: &str,
    #[case] line: usize,
) -> Result<()> {
    let text = format!("### Clause discharge\n| Clause | Discharge |\n| --- | --- |\n{rows}\n");
    let (_temp, repo) = fixture(FILE, &text)?;
    let error = discharged(&repo, FILE).expect_err("bad discharge must fail");
    let diagnostic = format!("{error:#}");
    ensure!(diagnostic.contains(expected), "{diagnostic}");
    if expected == "has no discharge column" {
        ensure!(diagnostic.contains(&format!("line {line}")), "{diagnostic}");
    } else {
        ensure!(
            diagnostic.contains(&format!("{FILE}:{line}")),
            "{diagnostic}"
        );
    }
    Ok(())
}

#[test]
fn a_discharge_returns_the_clause_set() -> Result<()> {
    let (_temp, repo) = fixture(
        FILE,
        "### Clause discharge\n| Clause | Discharge |\n| --- | --- |\n| `6.1` | `alpha` is pure. |",
    )?;
    {
        let actual = discharged(&repo, FILE)?;
        let expected_value = BTreeSet::from(["6.1".into()]);
        ensure!(
            actual == expected_value,
            "expected {expected_value:?}, found {actual:?}"
        );
    };
    Ok(())
}

#[rstest]
#[case::empty("### 5.1. Registry\n", "empty body", 2)]
#[case::generic("### 5.1. Registry\nGeneric obligations apply.", "names none", 2)]
#[case::foreign_helper("### 5.1. Registry\n`beta` is pure.", "names none", 2)]
#[case::fenced_helper(
    "### 5.1. Registry\n```\n`alpha`\n```\nGeneric prose.",
    "names none",
    2
)]
#[case::ansible(
    "### 5.1. Registry\n`alpha` behaves as Ansible does.",
    "appealing to Ansible",
    2
)]
#[case::duplicate(
    "### 5.1. Registry\n`alpha` is pure.\n### 5.1. Duplicate\n`alpha` is pure.",
    "second section 5 subsection",
    4
)]
fn vacuous_subsections_report_source_lines(
    #[case] body: &str,
    #[case] expected: &str,
    #[case] line: usize,
) -> Result<()> {
    let owned = BTreeSet::from(["alpha".into()]);
    let text = format!("## 5. Cross-cutting contract conformance\n{body}\n");
    let (_temp, repo) = fixture(FILE, &text)?;
    let error = check_section_five(&repo, FILE, &owned).expect_err("vacuity must fail");
    let diagnostic = error.to_string();
    ensure!(diagnostic.contains(expected), "{diagnostic}");
    ensure!(
        diagnostic.contains(&format!("{FILE}:{line}")),
        "{diagnostic}"
    );
    Ok(())
}

#[test]
fn missing_section_five_is_rejected() -> Result<()> {
    let (_temp, repo) = fixture(FILE, "## 4. Design")?;
    let error =
        check_section_five(&repo, FILE, &BTreeSet::new()).expect_err("missing section must fail");
    ensure!(error.to_string().contains("has no section 5"), "{error}");
    Ok(())
}

#[test]
fn explicit_escape_and_substantive_owned_helper_bodies_are_accepted() -> Result<()> {
    let text = concat!(
        "## 5. Cross-cutting contract conformance\n",
        "### 5.1. Registry\n`alpha` is pure.\n",
        "### 5.2. Namespace\nNo additional obligation beyond RFC 0006 section 6.2.\n",
        "### Worked example\nNot a numbered obligation.\n",
        "### Clause discharge\nThe separately checked table.\n",
    );
    let owned = BTreeSet::from(["alpha".into()]);
    let (_temp, repo) = fixture(FILE, text)?;
    {
        let actual = check_section_five(&repo, FILE, &owned)?;
        let expected_value = BTreeSet::from(["6.1".into(), "6.2".into()]);
        ensure!(
            actual == expected_value,
            "expected {expected_value:?}, found {actual:?}"
        );
    };
    Ok(())
}

#[rstest]
#[case("5.1. Registry", Some("6.1"))]
#[case("Worked example", None)]
#[case("5.1.2. Nested", None)]
#[case("five.1. Registry", None)]
#[case("5.one. Registry", None)]
#[case("5 Registry", None)]
fn subsection_ids_require_numeric_pair_shape(
    #[case] heading: &str,
    #[case] expected: Option<&str>,
) {
    assert_eq!(clause_id_of(heading).as_deref(), expected);
}

#[rstest]
#[case("`alpha` is pure", true)]
#[case("` alpha ` is pure", true)]
#[case("alpha is pure", false)]
#[case("`alphabet` is pure", false)]
#[case("`alpha()` is pure", false)]
#[case("`beta` is pure", false)]
fn helper_justification_requires_whole_code_tokens(#[case] body: &str, #[case] expected: bool) {
    assert_eq!(
        names_an_owned_helper(body, &BTreeSet::from(["alpha".into()])),
        expected
    );
}
