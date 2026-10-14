//! Exercise survey decisions, rename witnesses, and option evidence directly.
use super::*;
use anyhow::{bail, ensure};
use rstest::rstest;

/// Build one ordinary four-column survey table.
fn matrix(disposition: &str, resolution: &str) -> String {
    format!(
        "### 7.1. Core filters\n| Name | Alias | Decision | Resolution |\n|---|---|---|---|\n| `alpha` | a | {disposition} | {resolution} |\n"
    )
}

/// Build the smallest survey with all rename and option witnesses.
fn evidence() -> String {
    let mut text = matrix("Accept", "§8.1.");
    text.push_str(concat!(
        "| `hash` | h | Accept as `text_hash` | §8.9 |\n",
        "| `quote` | q | Reject | replacement |\n",
        "| `win_splitdrive` | w | Reject | replacement |\n",
        "| `basename` | b | Reject | §8.6 |\n",
        "| `dirname` | d | Reject | §8.6 |\n",
        "| `fileglob` | f | Reject | §8.7 |\n"
    ));
    text
}

#[rstest]
#[case::unknown(
    "Maybe",
    "§8.1",
    "unknown disposition \"Maybe\" at docs/rfcs/0006-ansible-inspired-template-standard-library.md:4"
)]
#[case::absent_citation(
    "Accept",
    "none",
    "accept row at docs/rfcs/0006-ansible-inspired-template-standard-library.md:4 cites no section 8 subsection"
)]
#[case::empty_citation("Accept", "§.", "cites no section 8 subsection")]
fn disposition_errors_report_source(
    #[case] disposition: &str,
    #[case] resolution: &str,
    #[case] diagnostic: &str,
) {
    let text = matrix(disposition, resolution);
    let error = read_section_7(&Section::whole(&text))
        .err()
        .expect("invalid disposition");
    assert!(format!("{error:#}").contains(diagnostic), "{error:#}");
}

#[rstest]
#[case::missing_disposition("| `alpha` | a |", "disposition")]
#[case::missing_resolution("| `alpha` | a | Accept |", "resolution")]
fn narrow_rows_report_missing_column(#[case] replacement: &str, #[case] column: &str) {
    let text = matrix("Accept", "§8.1").replace("| `alpha` | a | Accept | §8.1 |", replacement);
    let error = read_section_7(&Section::whole(&text))
        .err()
        .expect("narrow row");
    assert!(format!("{error:#}").contains(&format!("row at line 4 has no {column} column")));
}

#[test]
fn duplicate_survey_names_must_agree_on_namespace() {
    let mut text = matrix("Accept", "§8.1");
    text.push_str("### 7.4. Core tests\n| Name | Alias | Decision | Resolution |\n|---|---|---|---|\n| `alpha` | a | Reject | none |\n");
    let error = read_section_7(&Section::whole(&text))
        .err()
        .expect("conflicting namespaces");
    assert!(
        error
            .to_string()
            .contains("alpha is listed as both filter and test")
    );
}

#[test]
fn repeated_accept_rows_do_not_duplicate_new_registrations() {
    let text = matrix("Accept", "§8.1") + "| `alpha` | a | Accept | §8.1 |\n";
    let read = read_section_7(&Section::whole(&text)).expect("matching namespace");
    assert_eq!(read.accept_rows, 2);
    assert_eq!(read.new_rows.len(), 1);
}

#[rstest]
#[case::no_row("hash", "rename exception hash has no section 7 row", "row")]
#[case::missing_acceptance("text_hash", "only `hash` should already be", "accepted")]
#[case::unexpected_acceptance("shell_quote", "only `hash` should already be", "insert")]
#[case::missing_namespace("quote", "rename exception quote has no namespace", "namespace")]
fn rename_rejections_identify_missing_witness(
    #[case] name: &str,
    #[case] diagnostic: &str,
    #[case] mutation: &str,
) -> Result<()> {
    let text = evidence();
    let mut read = read_section_7(&Section::whole(&text))?;
    match mutation {
        "row" => {
            read.surveyed_names.remove(name);
        }
        "accepted" => {
            read.accepted.remove(name);
        }
        "insert" => {
            read.accepted.insert(
                name.into(),
                Row {
                    name: name.into(),
                    namespace: Namespace::Filter,
                },
            );
        }
        "namespace" => {
            read.namespaces.remove(name);
        }
        _ => bail!("unknown fixture mutation {mutation}"),
    }
    let error = apply_renames(&mut read)
        .err()
        .context("invalid rename witness must fail")?;
    ensure!(
        error.to_string().contains(diagnostic),
        "expected {diagnostic:?}, got {error}"
    );
    Ok(())
}

#[rstest]
#[case::missing_row("row", "row citing optioned helper basename does not exist")]
#[case::missing_citation(
    "citation",
    "row naming optioned helper basename cites no section 8 subsection"
)]
#[case::already_accepted("accepted", "optioned helper basename is already recorded as accepted")]
fn option_rejections_identify_missing_witness(
    #[case] mutation: &str,
    #[case] diagnostic: &str,
) -> Result<()> {
    let text = evidence();
    let mut read = read_section_7(&Section::whole(&text))?;
    match mutation {
        "row" => {
            read.surveyed_names.remove("basename");
        }
        "citation" => {
            read.cited.remove("basename");
        }
        "accepted" => {
            read.accepted.insert(
                "basename".into(),
                Row {
                    name: "basename".into(),
                    namespace: Namespace::Filter,
                },
            );
        }
        _ => bail!("unknown fixture mutation {mutation}"),
    }
    let error = apply_optioned(&mut read)
        .err()
        .context("invalid option witness must fail")?;
    ensure!(
        error.to_string().contains(diagnostic),
        "expected {diagnostic:?}, got {error}"
    );
    Ok(())
}

#[test]
fn valid_evidence_preserves_rename_and_option_namespaces() -> Result<()> {
    let text = evidence();
    let mut read = read_section_7(&Section::whole(&text))?;
    apply_renames(&mut read)?;
    let optioned = apply_optioned(&mut read)?;
    ensure!(
        optioned == ["basename", "dirname", "glob"],
        "unexpected optioned helpers {optioned:?}"
    );
    let glob = read.accepted.get("glob").context("glob registration")?;
    ensure!(
        glob.namespace == Namespace::Function,
        "glob must be a function"
    );
    ensure!(
        read.sections_of
            .get("splitdrive")
            .context("splitdrive section")?
            == "8.6",
        "splitdrive must own section 8.6"
    );
    Ok(())
}

#[rstest]
#[case("ACCEPT", Some(Disposition::Accept))]
#[case("Accept as `new`", Some(Disposition::Accept))]
#[case("Defer", Some(Disposition::Defer))]
#[case("Reject as redundant", Some(Disposition::Reject))]
#[case("Reject", Some(Disposition::Reject))]
#[case("Accepted", None)]
fn classify_recognizes_only_disposition_vocabulary(
    #[case] text: &str,
    #[case] expected: Option<Disposition>,
) {
    assert_eq!(classify(text), expected);
}

#[rstest]
#[case::no_marker("8.1", None)]
#[case::empty("§.", None)]
#[case::sentence("see §8.1.", Some("8.1"))]
#[case::terminated("§8.2 text", Some("8.2"))]
fn citations_stop_at_non_numeric_text(#[case] text: &str, #[case] expected: Option<&str>) {
    assert_eq!(section_ref(text).as_deref(), expected);
}

#[test]
fn registered_names_fall_back_when_accept_as_has_no_backticks() {
    let names = vec!["original".to_owned()];
    assert_eq!(registered_names("Accept as replacement", &names), names);
    assert_eq!(registered_names("Accept as `new`", &names), ["new"]);
}

#[rstest]
#[case::core(0)]
#[case::collection_filters(1)]
#[case::url(2)]
#[case::core_tests(3)]
#[case::filesystem(4)]
#[case::collection_tests(5)]
#[case::functions(6)]
fn inventory_tables_select_the_declared_column_and_namespace(#[case] index: usize) -> Result<()> {
    let table = CANDIDATE_TABLES
        .get(index)
        .context("candidate table index")?;
    let leading = if table.disposition_column == 1 {
        ""
    } else {
        " alias |"
    };
    let text = format!(
        "### {}\n| Header |\n|---|\n| `helper` |{leading} Accept | §8.1 |\n",
        table.heading
    );
    let read = read_section_7(&Section::whole(&text))?;
    let helper = read.accepted.get("helper").context("helper registration")?;
    ensure!(
        helper.namespace == table.namespace,
        "wrong namespace for {}",
        table.heading
    );
    ensure!(
        read.sections_of.get("helper").context("helper section")? == "8.1",
        "wrong helper section"
    );
    Ok(())
}

#[test]
fn irrelevant_tables_are_not_survey_candidates() -> Result<()> {
    let text = "### unrelated\n| h |\n|---|\n| malformed |";
    let read = read_section_7(&Section::whole(text))?;
    ensure!(
        read.surveyed_names.is_empty(),
        "unrelated table must contribute no names"
    );
    ensure!(
        read.reject_rows == 0,
        "unrelated table must contribute no reject rows"
    );
    Ok(())
}
