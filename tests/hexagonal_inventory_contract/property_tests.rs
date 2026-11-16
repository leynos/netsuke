//! Property-test the Markdown region and harvest rules.
//!
//! The example cases in `parser_tests.rs` pin regions a reader wrote by hand.
//! These properties cover the input domain instead: any document assembled from
//! headings at depths one to six, numbered or not, interleaved with prose and
//! indented fenced blocks. Each property restates a rule the parser claims in
//! prose, so a change that relaxes that rule fails here even when no
//! handwritten example happens to sit on the boundary.
//!
//! The generated fences are indented, matching the domain `is_heading`
//! documents: it tests a line prefix, so an unindented fence containing a `#`
//! line would read as a heading and is outside the rule it implements.

use proptest::prelude::*;

use super::markdown::{HeadingLevel, harvest_body, heading_body, is_heading};

/// One generated heading: its depth, its title, and the line that renders it.
type Generated = (u8, String, String);

/// Unwrap a lookup the generator guarantees, failing the property otherwise.
///
/// Every generated document contains the heading being requested, so a `None`
/// is a regression in the parser rather than an input to skip. Using `else {
/// continue }` here would make the whole property vacuous against exactly that
/// regression: a parser that always returned `None` would skip every case and
/// pass.
macro_rules! prop_assert_some {
    ($lookup:expr, $message:literal $(, $argument:expr)*) => {
        match $lookup {
            Some(value) => value,
            None => {
                prop_assert!(false, $message $(, $argument)*);
                unreachable!("prop_assert! returns from the test on failure")
            }
        }
    };
}

/// Render one Markdown heading line, numbering it when asked.
fn heading_line(depth: u8, index: usize, title: &str, is_numbered: bool) -> String {
    let hashes = "#".repeat(usize::from(depth));
    if is_numbered {
        format!("{hashes} {index}. {title}\n")
    } else {
        format!("{hashes} {title}\n")
    }
}

/// Part of a rendered document: its heading and the prose beneath it.
fn body_after_heading() -> String {
    concat!(
        "Some prose that makes no claim.\n",
        "\n",
        "```text\n",
        "  # indented, so not a heading\n",
        "```\n",
        "\n",
    )
    .to_owned()
}

/// Generate a document and the heading each block introduced.
///
/// Every title is unique because it carries the block index: `heading_body`
/// returns the first match, so a duplicated title would make a lookup find a
/// different region from the one the property means to test.
fn document() -> impl Strategy<Value = (String, Vec<Generated>)> {
    prop::collection::vec((1u8..=6, any::<bool>(), "[A-Za-z]{0,10}"), 0..10).prop_map(|blocks| {
        let mut text = String::new();
        let mut headings = Vec::new();
        for (index, (depth, is_numbered, stem)) in blocks.into_iter().enumerate() {
            let title = format!("{stem}{index}");
            let line = heading_line(depth, index, &title, is_numbered);
            text.push_str(&line);
            text.push_str(&body_after_heading());
            headings.push((depth, title, line));
        }
        (text, headings)
    })
}

/// Render the headings that follow position `index` in the same region.
///
/// A region ends at the next heading at or above its own level, so collecting
/// only the deeper headings before that boundary states what a correct region
/// slice must retain.
fn deeper_headings_before_boundary(
    headings: &[Generated],
    index: usize,
    boundary_depth: u8,
) -> Vec<String> {
    headings
        .iter()
        .skip(index + 1)
        .take_while(|(depth, _, _)| *depth > boundary_depth)
        .map(|(_, _, line)| line.clone())
        .collect()
}

/// Render the headings after `index` that a region at `boundary_depth` must
/// stop before.
///
/// The oracle is the generator's own record of each heading's depth and
/// rendered line, never the parser's predicate. These two properties once
/// asked `is_heading_at_or_above` about every line of the region, which is the
/// same function the reader under test calls: a mutation making it always
/// return false let the reader absorb every later section while the assertion
/// still saw no boundary, so both properties passed against a broken reader.
/// A property must not ask the code under test whether its own answer is right.
fn headings_at_or_above_after(
    headings: &[Generated],
    index: usize,
    boundary_depth: u8,
) -> Vec<String> {
    headings
        .iter()
        .skip(index + 1)
        .filter(|(depth, _, _)| *depth <= boundary_depth)
        .map(|(_, _, line)| line.clone())
        .collect()
}

proptest! {
    /// A region slice retains the deeper headings inside it.
    ///
    /// This is the rule `heading_body` states for `## Register` and its nested
    /// `####` area tables: truncating at the first heading of any depth would
    /// drop content that belongs to the region, and this property fails on it.
    #[test]
    fn section_regions_retain_every_nested_heading((text, headings) in document()) {
        for (index, (depth, title, _)) in headings.iter().enumerate() {
            if *depth != 2 {
                continue;
            }
            let region = prop_assert_some!(
                heading_body(&text, title, HeadingLevel::Section),
                "generated heading {title:?} should have a section region"
            );
            for nested in deeper_headings_before_boundary(&headings, index, 2) {
                prop_assert!(
                    region.contains(&nested),
                    "region for {title} should retain nested heading {nested:?}; got {region:?}"
                );
            }
        }
    }

    /// A region slice stops at the next heading at or above its own level.
    ///
    /// The complement of the retention rule: absorbing the next sibling section
    /// would make one region's claims readable as another's. The headings the
    /// region must exclude come from the generator, not from the reader's own
    /// predicate — see `headings_at_or_above_after`.
    #[test]
    fn section_regions_stop_at_the_next_sibling_heading((text, headings) in document()) {
        for (index, (depth, title, _)) in headings.iter().enumerate() {
            if *depth != 2 {
                continue;
            }
            let region = prop_assert_some!(
                heading_body(&text, title, HeadingLevel::Section),
                "generated heading {title:?} should have a section region"
            );
            for boundary in headings_at_or_above_after(&headings, index, 2) {
                prop_assert!(
                    !region.contains(&boundary),
                    "region for {title} should stop before {boundary:?}; got {region:?}"
                );
            }
        }
    }

    /// A subsection region stops at the next heading at or above depth three.
    #[test]
    fn subsection_regions_stop_at_the_next_sibling_heading((text, headings) in document()) {
        for (index, (depth, title, _)) in headings.iter().enumerate() {
            if *depth != 3 {
                continue;
            }
            let region = prop_assert_some!(
                heading_body(&text, title, HeadingLevel::Subsection),
                "generated heading {title:?} should have a subsection region"
            );
            for boundary in headings_at_or_above_after(&headings, index, 3) {
                prop_assert!(
                    !region.contains(&boundary),
                    "region for {title} should stop before {boundary:?}; got {region:?}"
                );
            }
        }
    }

    /// A harvest keeps no heading line at all, and prefixes its own region.
    ///
    /// Both halves matter. Dropping the truncation would let a deeper
    /// subheading's prose make a coverage claim; truncating anywhere but the
    /// first heading would make the harvest cut into text the region retains.
    #[test]
    fn harvests_are_heading_free_prefixes_of_their_region((text, headings) in document()) {
        for (_, title, _) in headings.iter().filter(|(depth, _, _)| *depth == 3) {
            let harvest = prop_assert_some!(
                harvest_body(&text, title),
                "generated heading {title:?} should have a harvest body"
            );
            prop_assert!(
                !harvest.lines().any(is_heading),
                "harvest for {title} should contain no heading line; got {harvest:?}"
            );
            let region = prop_assert_some!(
                heading_body(&text, title, HeadingLevel::Subsection),
                "generated heading {title:?} should have a subsection region"
            );
            prop_assert!(
                region.starts_with(harvest),
                "harvest for {title} should prefix its region; harvest {harvest:?}, region {region:?}"
            );
        }
    }
}
