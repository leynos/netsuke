//! Exercise section boundaries and source locations without tracked documents.
use super::*;
use rstest::rstest;

#[test]
fn sections_skip_fenced_headings_and_stop_at_peer_boundaries() {
    let text = concat!(
        "# Intro\n```\n## Target\n```\n## Target\n",
        "### Child\nbody\n~~~\n# ignored\n~~~\nafter\n## Peer\nend\n"
    );
    let whole = Section::whole(text);
    let section = whole.subsection("## Target").expect("real target exists");
    assert_eq!(section.first_line, 5);
    assert_eq!(section.lines.last(), Some(&"after"));
    assert!(whole.subsection("## Missing").is_none());
    assert!(whole.subsection("## Tar").is_none());
    let child = section.subsection("### Child").expect("child exists");
    assert_eq!(child.first_line, 6);
    let subsections = section.subsections();
    assert_eq!(subsections.len(), 1);
    let first = subsections.first().expect("one subsection");
    assert_eq!(first.line, 6);
    assert_eq!(first.body, ["body", "after"]);
}

#[test]
fn table_recognition_preserves_absolute_lines_and_separate_headers() {
    let text = concat!(
        "# Intro\n## Tables\n### First\n| a | b |\n|---|:---:|\n",
        "| value | other |\n\n| header |\n| second |\n",
        "```\n### Fake\n| h |\n| fake |\n```\n### Empty\n| h |\n"
    );
    let section = Section::whole(text)
        .subsection("## Tables")
        .expect("tables exist");
    let tables = section.tables();
    assert_eq!(tables.len(), 2);
    let first = tables.first().expect("first table");
    let row = first.1.first().expect("first data row");
    assert_eq!(first.0, "First");
    assert_eq!(row.line, 6);
    assert_eq!(row.cell(1, "value").expect("second cell"), "other");
    let second = tables.get(1).expect("second table");
    assert_eq!(second.1.first().expect("second data row").line, 9);
}

#[rstest]
#[case::shallow("## peer")]
#[case::root("# root")]
#[case::same("### peer")]
fn subsection_ends_at_same_or_shallower_heading(#[case] boundary: &str) {
    let text = format!("### target\nbody\n#### nested\nmore\n{boundary}\nafter");
    let section = Section::whole(&text)
        .subsection("### target")
        .expect("target exists");
    assert_eq!(section.lines.last(), Some(&"more"));
}

#[test]
fn missing_raw_cell_reports_column_and_absolute_source_line() {
    let row = RawRow {
        cells: vec!["only".into()],
        line: 42,
    };
    let error = row.cell(1, "resolution").expect_err("missing cell");
    assert_eq!(error.to_string(), "row at line 42 has no resolution column");
}
