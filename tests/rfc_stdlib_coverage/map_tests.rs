//! Direct coverage-map diagnostics and independent ownership set properties.

use super::*;
use proptest::prelude::*;
use proptest::test_runner::{FileFailurePersistence, TestCaseError};
use rstest::rstest;

/// Build a one-section inventory with two distinct helpers.
fn sections() -> BTreeMap<String, Vec<String>> {
    BTreeMap::from([("8.1".into(), vec!["alpha".into(), "beta".into()])])
}

/// Parse a synthetic row through the document layer to preserve source lines.
fn row(cells: &str) -> RawRow {
    let text = format!("# Fixture\n| header |\n| --- |\n{cells}\n");
    Section::whole(&text).tables().remove(0).1.remove(0)
}

#[rstest]
#[case::short("13")]
#[case::suffix("0013b")]
#[case::unicode("٠٠١٣")]
#[case::bad_link("[0013](missing")]
#[case::bad_label("[RFC 0013](0013-child.md)")]
fn child_numbers_reject_malformed_cells(#[case] input: &str) {
    assert!(child_number(input).is_none());
}

#[test]
fn child_numbers_accept_bare_and_linked_cells() {
    assert_eq!(child_number("`0013`"), Some(("0013".into(), None)));
    assert_eq!(
        child_number("[0013](0013-child.md)"),
        Some(("0013".into(), Some("0013-child.md".into())))
    );
}

#[rstest]
#[case::number(
    "| bad | group | `8.1` | — | 6.2 | unwritten |",
    "unreadable child RFC cell"
)]
#[case::status(
    "| `0013` | group | `8.1` | — | 6.2 | draft |",
    "expected `written` or `unwritten`"
)]
#[case::missing_link("| `0013` | group | `8.1` | — | 6.2 | written |", "marked written")]
#[case::unexpected_link(
    "| [0013](0013-child.md) | group | `8.1` | — | 6.2 | unwritten |",
    "marked unwritten"
)]
#[case::wrong_target(
    "| [0013](0014-child.md) | group | `8.1` | — | 6.2 | written |",
    "whose number is 0014"
)]
#[case::non_rfc_target(
    "| [0013](child.md) | group | `8.1` | — | 6.2 | written |",
    "not an RFC number"
)]
#[case::owns_repeat(
    "| `0013` | group | `8.1`; `8.1` | — | 6.2 | unwritten |",
    "`Owns` cell"
)]
#[case::option_repeat(
    "| `0013` | group | `8.1` | `glob` / `glob` | 6.2 | unwritten |",
    "`Optioned` cell"
)]
fn map_rows_reject_one_bad_condition(#[case] input: &str, #[case] expected: &str) {
    let error = parse_row(&row(input), &sections())
        .err()
        .expect("invalid row must fail");
    let diagnostic = format!("{error:#}");
    assert!(diagnostic.contains(expected), "{diagnostic}");
    if !expected.starts_with("marked") {
        assert!(
            diagnostic.contains(&format!("{RFC_0006}:4")),
            "{diagnostic}"
        );
    }
}

#[rstest]
#[case::no_section("plain", "names no section")]
#[case::unknown_section("`8.99`", "specifies no accepted helper")]
#[case::bare_arity("`8.1` `alpha`", "extra backticked tokens")]
#[case::missing_member("`8.1` only missing", "takes a section and one member")]
#[case::extra_member("`8.1` except `alpha` `beta`", "takes a section and one member")]
#[case::unknown_only("`8.1` only `missing`", "includes missing")]
#[case::unknown_except("`8.1` except `missing`", "excludes missing")]
#[case::empty(" ; ", "claims no helper")]
fn ownership_clauses_reject_invalid_inputs(#[case] input: &str, #[case] expected: &str) {
    let error = resolve_owns(input, &sections(), 42).expect_err("invalid ownership must fail");
    let diagnostic = format!("{error:#}");
    assert!(diagnostic.contains(expected), "{diagnostic}");
    assert!(
        diagnostic.contains(&format!("{RFC_0006}:42")),
        "{diagnostic}"
    );
}

#[test]
fn excluding_the_only_member_is_empty_ownership() {
    let inventory = BTreeMap::from([("8.1".into(), vec!["alpha".into()])]);
    let error =
        resolve_owns("`8.1` except `alpha`", &inventory, 9).expect_err("empty result must fail");
    assert!(error.to_string().contains("claims no helper"));
}

#[test]
fn ownership_detects_cross_child_claims_but_allows_owned_option_overlap() {
    let first = parse_row(
        &row("| `0013` | group | `8.1` | `alpha` | 6.2 | unwritten |"),
        &sections(),
    )
    .expect("valid overlap");
    let second = parse_row(
        &row("| `0014` | group | `8.1` | — | 6.3 | unwritten |"),
        &sections(),
    )
    .expect("valid second row");
    assert_eq!(first.claims().len(), 3);
    let one = Map { rows: vec![first] };
    assert_eq!(
        one.ownership()
            .expect("same child overlap is allowed")
            .len(),
        2
    );
    let mut two = one;
    two.rows.push(second);
    let error = two.ownership().expect_err("cross-child claim must fail");
    assert!(
        error
            .to_string()
            .contains("alpha is claimed by both RFC 0013 and RFC 0014")
    );
    assert_eq!(two.unwritten(), 2);
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(
            "tests/rfc_stdlib_coverage/map_tests.proptest-regressions",
        ))),
        ..ProptestConfig::default()
    })]

    #[test]
    fn owns_matches_independent_set_operations(
        members in proptest::collection::btree_set(0u8..16, 2..9),
        section in 1u8..11,
        choice in 0usize..32,
        mode in 0u8..3,
    ) {
        let universe: BTreeSet<String> = members.iter().map(|id| format!("helper_{id}")).collect();
        let selected = universe.iter().nth(choice.checked_rem(universe.len()).ok_or_else(|| TestCaseError::fail("non-zero inventory length"))?).ok_or_else(|| TestCaseError::fail("non-empty inventory"))?;
        let key = format!("8.{section}");
        let inventory = BTreeMap::from([(key.clone(), universe.iter().cloned().collect())]);
        let (clause, expected) = match mode {
            0 => (format!("`{key}`"), universe.clone()),
            1 => (format!("`{key}` only `{selected}`"), BTreeSet::from([selected.clone()])),
            _ => (format!("`{key}` except `{selected}`"), universe.difference(&BTreeSet::from([selected.clone()])).cloned().collect()),
        };
        let actual = resolve_owns(&clause, &inventory, 17).map_err(|error| TestCaseError::fail(error.to_string()))?;
        prop_assert_eq!(actual.into_iter().collect::<BTreeSet<_>>(), expected);
    }

    #[test]
    fn owns_rejects_generated_arity_and_unknown_references(
        section in 1u8..11,
        member in 0u8..16,
        invalid in 0u8..5,
    ) {
        let key = format!("8.{section}");
        let helper = format!("helper_{member}");
        let inventory = BTreeMap::from([(key.clone(), vec![helper.clone()])]);
        let (clause, expected) = match invalid {
            0 => (format!("`{key}` only `{helper}` `extra`"), "takes a section and one member"),
            1 => (format!("`{key}` except missing"), "takes a section and one member"),
            2 => (format!("`{key}` `{helper}`"), "extra backticked tokens"),
            3 => ("`8.99`".to_owned(), "specifies no accepted helper"),
            _ => (format!("`{key}` only `unknown`"), "includes unknown"),
        };
        let result = resolve_owns(&clause, &inventory, 17);
        prop_assert!(result.is_err());
        let diagnostic = result.err().ok_or_else(|| TestCaseError::fail("rejection expected"))?.to_string();
        prop_assert!(diagnostic.contains(expected), "{diagnostic}");
        let source_context = format!("{RFC_0006}:17");
        prop_assert!(diagnostic.contains(&source_context), "{diagnostic}");
    }
}

/// Write a coverage-map document inside an isolated repository fixture.
fn map_fixture(text: &str) -> Result<(tempfile::TempDir, Repo)> {
    let temp = tempfile::tempdir()?;
    let root = camino::Utf8Path::from_path(temp.path()).context("UTF-8 fixture path")?;
    let repo = Repo::fixture(root)?;
    repo.dir.create_dir_all("docs/rfcs")?;
    repo.dir.write(RFC_0006, text)?;
    Ok((temp, repo))
}

/// Render eight distinct reservations, changing only the final number on request.
fn map_document(duplicate: bool) -> String {
    let rows = (0..8)
        .map(|index| {
            let number = if duplicate && index == 7 {
                13
            } else {
                13 + index
            };
            format!("| `{number:04}` | group | `8.1` | — | 6.2 | unwritten |\n")
        })
        .collect::<Vec<_>>()
        .concat();
    format!(
        "### 14.13. Coverage map\n| RFC | Group | Owns | Optioned | Step | Status |\n| --- | --- | --- | --- | --- | --- |\n{rows}"
    )
}

#[rstest]
#[case::missing_heading("## 14. Delivery", "contains no coverage map table")]
#[case::missing_table("### 14.13. Coverage map\nProse only", "subsection contains no table")]
#[case::wrong_count(
    "### 14.13. Coverage map\n| RFC | Group | Owns | Optioned | Step | Status |\n| --- | --- | --- | --- | --- | --- |\n| `0013` | group | `8.1` | — | 6.2 | unwritten |",
    "has 1 rows; expected 8"
)]
fn map_structure_rejects_missing_and_partial_tables(
    #[case] text: &str,
    #[case] expected: &str,
) -> Result<()> {
    let (_temp, repo) = map_fixture(text)?;
    let error = parse(&repo, &sections())
        .err()
        .context("bad map must fail")?;
    ensure!(error.to_string().contains(expected), "{error}");
    Ok(())
}

#[test]
fn duplicate_reservations_fail_before_ownership_is_checked() -> Result<()> {
    let (_temp, repo) = map_fixture(&map_document(true))?;
    let error = parse(&repo, &sections())
        .err()
        .context("duplicate reservation must fail")?;
    ensure!(
        error
            .to_string()
            .contains("reserves RFC 0013 more than once"),
        "{error}"
    );
    Ok(())
}

#[test]
fn eight_distinct_reservations_parse() -> Result<()> {
    let (_temp, repo) = map_fixture(&map_document(false))?;
    {
        let actual = parse(&repo, &sections())?.rows.len();
        let expected_value = 8;
        ensure!(
            actual == expected_value,
            "expected {expected_value:?}, found {actual:?}"
        );
    };
    Ok(())
}

#[test]
fn written_rows_preserve_link_step_and_option_claims() {
    let parsed = parse_row(
        &row("| [0013](0013-child.md) | group | `8.1` only `alpha` | `glob` | 6.2 | written |"),
        &sections(),
    )
    .expect("written row agrees with its numbered link");
    assert_eq!(parsed.written.as_deref(), Some("0013-child.md"));
    assert_eq!(parsed.step, "6.2");
    assert_eq!(parsed.claims(), ["alpha", "glob"]);
    assert_eq!(Map { rows: vec![parsed] }.unwritten(), 0);
}

#[test]
fn missing_status_column_reports_the_source_line() {
    let error = parse_row(&row("| `0013` | group | `8.1` | — | 6.2 |"), &sections())
        .err()
        .expect("narrow row must fail");
    assert!(format!("{error:#}").contains("row at line 4 has no status column"));
}
