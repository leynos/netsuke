//! Exercise the Markdown and roadmap parsers with small fixtures.

use std::collections::BTreeSet;

use anyhow::{Result, ensure};
use pretty_assertions::assert_eq;
use rstest::rstest;

use super::fixtures::{HEADING_BLEED_ROADMAP, PIN, TRANSITIVE_ROADMAP};
use super::markdown::{HeadingLevel, harvest_body, heading_body, pins, register_ids, task_ids};
use super::roadmap::{dependents_of, roadmap_tasks};

#[rstest]
#[case::plain_heading("## Purpose\nbody\n\n## Next\nother\n", "Purpose", "body\n\n")]
#[case::numbered_heading("## 4. Register\nrows\n## 5. Next\n", "Register", "rows\n")]
fn section_stops_at_the_next_second_level_heading(
    #[case] source: &str,
    #[case] heading: &str,
    #[case] expected: &str,
) -> Result<()> {
    ensure!(
        heading_body(source, heading, HeadingLevel::Section) == Some(expected),
        "section body differs"
    );
    Ok(())
}

#[test]
fn heading_bodies_keep_nested_subsection_content() -> Result<()> {
    let source = concat!(
        "### A1\n\n",
        "#### A1 type rows\n| A1/x |\n",
        "#### A1 site rows\n| A1/y |\n",
    );
    let body = heading_body(source, "A1", HeadingLevel::Subsection);
    ensure!(
        body.is_some_and(|text| text.contains("A1 type rows") && text.contains("A1 site rows")),
        "nested subsections should stay inside the parent body; got {body:?}"
    );
    Ok(())
}

#[test]
fn harvest_bodies_stop_at_the_first_nested_heading() -> Result<()> {
    let source = "### Changes\nbody\n#### Notes\nleaked\n### Consumes\nmore\n";
    let body = harvest_body(source, "Changes");
    ensure!(
        body == Some("body\n"),
        "harvest body should stop at the nested heading; got {body:?}"
    );
    Ok(())
}

#[rstest]
#[case::lowercase_sha(format!("{PIN} {PIN}g {} {}", "A".repeat(40), "a".repeat(39)), 1)]
#[case::no_sha("no commit here".to_owned(), 0)]
fn pins_collect_only_lowercase_forty_character_tokens(
    #[case] source: String,
    #[case] expected: usize,
) -> Result<()> {
    ensure!(
        pins(&source).len() == expected,
        "unexpected pin-token count"
    );
    Ok(())
}

#[rstest]
#[case::version_and_two_part_label("0.1.0 and 28.2", &[])]
#[case::slash_joined("29.2.1/29.2.2", &["29.2.1", "29.2.2"])]
fn task_ids_ignore_versions_and_two_part_labels(#[case] source: &str, #[case] expected: &[&str]) {
    let actual = task_ids(source);
    let expected_set: BTreeSet<String> = expected.iter().map(|value| (*value).to_owned()).collect();
    assert_eq!(actual, expected_set);
}

#[test]
fn roadmap_parser_reads_dependencies_and_transitive_continuations() {
    let tasks = roadmap_tasks(TRANSITIVE_ROADMAP);
    assert_eq!(
        tasks.get("26.1.3"),
        Some(&BTreeSet::from([
            "26.1.2".to_owned(),
            "29.2.1".to_owned(),
            "29.2.2".to_owned(),
            "27.1.1".to_owned(),
        ]))
    );
    assert_eq!(
        dependents_of(&tasks, "26.1.1"),
        BTreeSet::from(["26.1.2".to_owned(), "26.1.3".to_owned()])
    );
}

#[test]
fn dependency_continuations_stop_at_headings_and_unindented_prose() {
    let tasks = roadmap_tasks(HEADING_BLEED_ROADMAP);
    let boundary: BTreeSet<String> = ["26.2.3", "26.3.1"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    assert_eq!(tasks.get("26.3.2"), Some(&boundary));
    assert_eq!(
        tasks.get("27.1.1"),
        Some(&BTreeSet::from(["26.3.2".to_owned()]))
    );
}

#[test]
fn register_ids_read_first_cells_from_register_tables() {
    let source = concat!(
        "## Register\n",
        "| ID | Item |\n| -- | -- |\n",
        "| A1/ast::Rule | Rule |\n\n",
        "## Trace matrix\n",
        "| Task | Register items |\n| -- | -- |\n| 26.1.2 | A1/* |\n"
    );
    assert_eq!(register_ids(source), ["A1/ast::Rule"]);
}
