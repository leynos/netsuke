//! Isolated registry fixtures and per-cell vocabulary diagnostics.

use super::*;
use rstest::rstest;

const FILE: &str = "docs/rfcs/0013-child.md";

/// Write one isolated registry document, keeping its directory alive.
fn fixture(file: &str, text: &str) -> Result<(tempfile::TempDir, Repo)> {
    let temp = tempfile::tempdir()?;
    let root = camino::Utf8Path::from_path(temp.path()).context("UTF-8 fixture path")?;
    let repo = Repo::fixture(root)?;
    repo.dir.create_dir_all("docs/rfcs")?;
    repo.dir.write(file, text)?;
    Ok((temp, repo))
}

/// Build a registry whose data row starts at line five.
fn registry(cells: &str) -> String {
    format!(
        "# Child\n### 5.1. Registry\n| Helper | Namespace | Registration | Purity | Query |\n| --- | --- | --- | --- | --- |\n{cells}\n"
    )
}

#[rstest]
#[case::empty("| `` | filter | new | pure | yes |", "empty helper cell")]
#[case::namespace(
    "| `alpha` | macro | new | pure | yes |",
    "expected `filter`, `test`, or `function`"
)]
#[case::registration(
    "| `alpha` | filter | old | pure | yes |",
    "expected `new` or `option added`"
)]
#[case::purity(
    "| `alpha` | filter | new | ambient | yes |",
    "table 2 does not define"
)]
#[case::query("| `alpha` | filter | new | pure | maybe |", "expected `yes` or `no`")]
#[case::pure_disagreement("| `alpha` | filter | new | pure | no |", "table 2 says yes")]
#[case::impure_disagreement(
    "| `alpha` | filter | new | filesystem-observing | yes |",
    "table 2 says no"
)]
fn invalid_registry_cells_report_file_and_line(
    #[case] cells: &str,
    #[case] expected: &str,
) -> Result<()> {
    let (_temp, repo) = fixture(FILE, &registry(cells))?;
    let error = parse(&repo, FILE)
        .err()
        .context("invalid registry must fail")?;
    let diagnostic = format!("{error:#}");
    ensure!(diagnostic.contains(expected), "{diagnostic}");
    ensure!(diagnostic.contains(&format!("{FILE}:5")), "{diagnostic}");
    Ok(())
}

#[rstest]
#[case::missing_heading("# Child", "no registry table")]
#[case::empty_table("### 5.1. Registry\n| Helper |\n| --- |", "contains no table")]
#[case::no_table("### 5.1. Registry\nProse only", "contains no table")]
fn missing_registry_data_is_rejected(#[case] text: &str, #[case] expected: &str) -> Result<()> {
    let (_temp, repo) = fixture(FILE, text)?;
    let error = parse(&repo, FILE)
        .err()
        .context("missing registry must fail")?;
    ensure!(error.to_string().contains(expected), "{error}");
    ensure!(error.to_string().contains(FILE), "{error}");
    Ok(())
}

#[rstest]
#[case::malformed("docs/rfcs/child.md", "not named as an RFC")]
#[case::parent_number("docs/rfcs/0012-child.md", "child RFCs start at 0013")]
fn registry_filename_is_validated(#[case] file: &str, #[case] expected: &str) -> Result<()> {
    let (_temp, repo) = fixture(file, &registry("| `alpha` | filter | new | pure | yes |"))?;
    let error = parse(&repo, file).err().context("bad filename must fail")?;
    ensure!(error.to_string().contains(expected), "{error}");
    Ok(())
}

#[test]
fn duplicate_registry_names_report_the_second_row() -> Result<()> {
    let data = "| `alpha` | filter | new | pure | yes |";
    let (_temp, repo) = fixture(FILE, &registry(&format!("{data}\n{data}")))?;
    let error = parse(&repo, FILE).err().context("duplicate must fail")?;
    ensure!(
        error
            .to_string()
            .contains(&format!("{FILE}:6 registers alpha twice")),
        "{error}"
    );
    Ok(())
}

#[rstest]
#[case::filter("filter", Namespace::Filter)]
#[case::test("test", Namespace::Test)]
#[case::function("function", Namespace::Function)]
fn all_namespace_forms_parse(#[case] label: &str, #[case] expected: Namespace) -> Result<()> {
    let (_temp, repo) = fixture(
        FILE,
        &registry(&format!("| `alpha` | {label} | new | pure | yes |")),
    )?;
    let parsed = parse(&repo, FILE)?;
    {
        let actual = parsed
            .rows
            .first()
            .context("registry row exists")?
            .helper
            .namespace;
        let expected_value = expected;
        ensure!(
            actual == expected_value,
            "expected {expected_value:?}, found {actual:?}"
        );
    };
    Ok(())
}

#[rstest]
#[case::pure("pure", Purity::Pure, "yes")]
#[case::clock("clock-observing", Purity::Clock, "no")]
#[case::environment("environment-observing", Purity::Environment, "no")]
#[case::filesystem("filesystem-observing", Purity::Filesystem, "no")]
#[case::network("network-observing", Purity::Network, "no")]
#[case::subprocess("subprocess-observing", Purity::Subprocess, "no")]
fn purity_vocabulary_and_counts_exclude_option_rows(
    #[case] label: &str,
    #[case] expected: Purity,
    #[case] query: &str,
) -> Result<()> {
    let rows = format!(
        "| `alpha` | filter | new | {label} | {query} |\n| `beta` | filter | option added | {label} | {query} |"
    );
    let (_temp, repo) = fixture(FILE, &registry(&rows))?;
    let parsed = parse(&repo, FILE)?;
    {
        let actual = parsed.new_with_purity(expected);
        let expected_value = 1;
        ensure!(
            actual == expected_value,
            "expected {expected_value:?}, found {actual:?}"
        );
    };
    {
        let actual = parsed.with_registration(Registration::New);
        let expected_value = 1;
        ensure!(
            actual == expected_value,
            "expected {expected_value:?}, found {actual:?}"
        );
    };
    {
        let actual = parsed.with_registration(Registration::OptionAdded);
        let expected_value = 1;
        ensure!(
            actual == expected_value,
            "expected {expected_value:?}, found {actual:?}"
        );
    };
    {
        let actual = parsed.names();
        let expected_value = BTreeSet::from(["alpha".into(), "beta".into()]);
        ensure!(
            actual == expected_value,
            "expected {expected_value:?}, found {actual:?}"
        );
    };
    Ok(())
}

#[test]
fn parse_all_uses_only_reserved_numbers_and_sorts_them() -> Result<()> {
    let text = registry("| `alpha` | filter | new | pure | yes |");
    let (_temp, repo) = fixture("docs/rfcs/0014-child.md", &text)?;
    repo.dir.write(FILE, &text)?;
    repo.dir
        .write("docs/rfcs/0099-other.md", "not a registry")?;
    repo.dir.write("docs/rfcs/readme.md", "not numbered")?;
    repo.dir.write("docs/rfcs/0015-child.txt", "not Markdown")?;
    let parsed = parse_all(&repo, &["0014".into(), "0013".into()])?;
    {
        let actual = parsed
            .iter()
            .map(|registry| registry.number.as_str())
            .collect::<Vec<_>>();
        let expected_value = ["0013", "0014"];
        ensure!(
            actual == expected_value,
            "expected {expected_value:?}, found {actual:?}"
        );
    };
    Ok(())
}

#[rstest]
#[case("13-child.md", None)]
#[case("00133-child.md", None)]
#[case("٠٠١٣-child.md", None)]
#[case("docs/rfcs/0013-child.md", Some("0013"))]
fn filename_numbers_require_four_ascii_digits(#[case] input: &str, #[case] expected: Option<&str>) {
    assert_eq!(rfc_number(input).as_deref(), expected);
}

#[rstest]
#[case::namespace("| `alpha` |", "namespace")]
#[case::registration("| `alpha` | filter |", "registration")]
#[case::purity("| `alpha` | filter | new |", "purity class")]
#[case::query("| `alpha` | filter | new | pure |", "manifest query")]
fn narrow_registry_rows_report_the_missing_column(
    #[case] cells: &str,
    #[case] expected: &str,
) -> Result<()> {
    let (_temp, repo) = fixture(FILE, &registry(cells))?;
    let error = parse(&repo, FILE).err().context("narrow row must fail")?;
    let diagnostic = format!("{error:#}");
    ensure!(
        diagnostic.contains(&format!("row at line 5 has no {expected} column")),
        "{diagnostic}"
    );
    Ok(())
}
