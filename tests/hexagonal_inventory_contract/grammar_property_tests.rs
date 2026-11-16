//! Property-test the identifier grammar and the claim-binding relations.
//!
//! `property_tests.rs` covers the Markdown region readers. These properties
//! cover the rules those readers feed: the register grammar's closed sets, the
//! wildcard coverage relation, and the trace-row width contract. Each states a
//! rule the module documents in prose and holds it over generated input, so a
//! relaxation fails here even when no handwritten example sits on the boundary.
//!
//! The coverage properties test both directions on purpose. A coverage rule is
//! a relation, and it can fail by accepting a token that names nothing as
//! readily as by refusing a token that names its target. A property that only
//! checked acceptance would pass a rule which always answered `true`.

use proptest::prelude::*;

use super::obligations::{TRACE_COLUMNS, parse_trace_row};
use super::register::is_well_formed_register_id;
use super::trace::reference_covers;

/// The register areas the grammar's closed set permits.
const AREAS: [&str; 7] = ["A1", "A2", "A3", "A4", "A5", "A6", "A7"];

/// The site kinds the grammar's closed set permits.
const SITE_KINDS: [&str; 4] = ["constructor", "mutator", "consumer", "api-reachable"];

/// An area name, drawn half from the permitted set and half from elsewhere.
///
/// The closed set is the whole rule, so a generator drawing only members could
/// not tell "checks the area" from "accepts any area".
fn area() -> impl Strategy<Value = String> {
    prop_oneof![
        prop::sample::select(&AREAS[..]).prop_map(str::to_owned),
        "[A-Za-z][0-9]{0,2}",
    ]
}

/// A register item: a short word carrying neither delimiter.
///
/// `#` and `@` are excluded because the grammar reads both as delimiters;
/// generating them would make these properties assert about a malformed item
/// rather than about the shape under test.
fn item() -> impl Strategy<Value = String> {
    "[A-Za-z_][A-Za-z0-9_:]{0,8}"
}

/// A site kind, optionally numbered, or a word that is not a permitted kind.
fn site_kind() -> impl Strategy<Value = String> {
    prop_oneof![
        (prop::sample::select(&SITE_KINDS[..]), 0u8..3).prop_map(|(kind, number)| {
            if number == 0 {
                kind.to_owned()
            } else {
                format!("{kind}-{number}")
            }
        }),
        "[a-z]{1,10}",
    ]
}

/// Build a candidate identifier from its parts, keeping every delimiter.
fn compose(area: &str, item: &str, site: Option<(&str, &str)>) -> String {
    match site {
        Some((symbol, kind)) => format!("{area}/{item}@{symbol}#{kind}"),
        None => format!("{area}/{item}"),
    }
}

/// Unwrap a `Result` the generator guarantees, failing the property otherwise.
///
/// Every generated row has the declared width, so an `Err` here is a
/// regression in the reader rather than an input to skip. A `continue` or a
/// silent `unwrap_or_default()` would let a reader that refused every row
/// pass this property vacuously.
///
/// The `match` is needed rather than `let ... else`: inside a `proptest!`
/// body a bare `return` does not type-check, and the arm after a failing
/// `prop_assert!` has to produce a value of the scrutinee's type.
macro_rules! prop_assert_ok {
    ($expression:expr) => {
        match $expression {
            Ok(value) => value,
            Err(error) => {
                prop_assert!(false, "generated input should be accepted: {error}");
                unreachable!("prop_assert! returns from the test on failure")
            }
        }
    };
}

/// Report whether the documented grammar should accept an identifier.
///
/// The expectation is restated from the published rule — a permitted area, an
/// item that is non-empty and carries no `#`, and either no site or a non-empty
/// symbol before a permitted kind — rather than asked of the code under test.
/// An expectation computed with the reader's own predicate agrees with it
/// whatever it does, which is how the conditional this replaced came to assert
/// nothing at all.
fn should_accept(area: &str, item: &str, site: Option<(&str, &str)>) -> bool {
    let is_valid_item = !item.is_empty() && !item.contains('#');
    let is_valid_site = site.is_none_or(|(symbol, kind)| {
        !symbol.is_empty() && !has_delimiter(symbol) && is_documented_site_kind(kind)
    });
    AREAS.contains(&area) && is_valid_item && is_valid_site
}

/// Report whether a value carries a character that would make a split ambiguous.
fn has_delimiter(value: &str) -> bool {
    value
        .chars()
        .any(|character| matches!(character, '@' | '#'))
}

/// Report whether a kind is a permitted site kind, optionally numbered.
///
/// The documented rule admits a member of the closed set or that member
/// followed by `-<digits>`. Restating it rather than reusing the reader's own
/// `matches_site_kind` keeps the near miss `constructor-x` on the refusal side
/// of the expectation, so a reader that dropped the digit requirement has to
/// disagree with this function for the property to reject it.
fn is_documented_site_kind(kind: &str) -> bool {
    SITE_KINDS.iter().any(|known| {
        kind.strip_prefix(known).is_some_and(|suffix| {
            suffix.is_empty() || suffix.strip_prefix('-').is_some_and(is_all_digits)
        })
    })
}

/// Report whether a numbered suffix is a non-empty run of decimal digits.
fn is_all_digits(suffix: &str) -> bool {
    !suffix.is_empty() && suffix.chars().all(|character| character.is_ascii_digit())
}

proptest! {
    /// Well-formedness agrees with the documented grammar on generated parts.
    ///
    /// Both directions are asserted, which is the whole point. An earlier form
    /// wrapped its assertions in `if is_well_formed_register_id(…)`, so a reader
    /// that refused an identifier asserted nothing about it: a probe injecting
    /// an unconditional `false` left all eight grammar properties passing.
    /// Stating the expectation independently rejects that mutant, and the area
    /// guard written for a reader that accepted any area at all.
    #[test]
    fn well_formedness_agrees_with_the_documented_grammar(
        area in area(),
        item in item(),
        site in prop::option::of((item(), site_kind())),
    ) {
        let identifier = match &site {
            Some((symbol, kind)) => compose(&area, &item, Some((symbol, kind))),
            None => compose(&area, &item, None),
        };
        let parts = site
            .as_ref()
            .map(|(symbol, kind)| (symbol.as_str(), kind.as_str()));
        let expected = should_accept(&area, &item, parts);
        prop_assert_eq!(
            is_well_formed_register_id(&identifier),
            expected,
            "grammar should answer {} for {:?} built from area {:?} and site {:?}",
            expected,
            identifier,
            area,
            site
        );
    }

    /// An identifier with no area separator is refused.
    ///
    /// The area is the identifier's first component, so a string without `/`
    /// names no register item at all. Reading the whole value as an item would
    /// accept `A1` on its own.
    #[test]
    fn identifiers_without_an_area_are_refused(body in "[A-Za-z0-9@#_-]{0,12}") {
        prop_assert!(
            !is_well_formed_register_id(&body),
            "{body:?} names no area and should be refused"
        );
    }

    /// An exact identifier always covers itself.
    ///
    /// The direction a coverage rule must never lose: a trace row naming an
    /// item has traced it.
    #[test]
    fn an_identifier_covers_itself(area in area(), item in item()) {
        let identifier = compose(&area, &item, None);
        prop_assert!(
            reference_covers(&identifier, &identifier),
            "{identifier:?} should cover itself"
        );
    }

    /// An area wildcard covers its own prefix and nothing beside it.
    ///
    /// `covers_area` strips `/*` and asks whether the target *starts with* the
    /// remainder — a documented prefix rule, not an equality on the area. So
    /// the uncovered target must be one that does not begin with the prefix at
    /// all. Excluding only equal areas would assert a stronger rule than the
    /// code states and fail on a legitimate prefix: `_/*` does cover `__/A`.
    #[test]
    fn an_area_wildcard_covers_only_its_own_prefix(
        prefix in item(),
        other in item(),
        tail in item(),
    ) {
        let wildcard = format!("{prefix}/*");
        let inside = compose(&prefix, &tail, None);
        prop_assert!(
            reference_covers(&inside, &wildcard),
            "{wildcard:?} should cover {inside:?}"
        );
        prop_assume!(!other.starts_with(&prefix));
        let outside = compose(&other, &tail, None);
        prop_assert!(
            !reference_covers(&outside, &wildcard),
            "{wildcard:?} should not cover {outside:?}"
        );
    }

    /// A bare `/*` covers nothing.
    ///
    /// An empty prefix is a prefix of every string, so this shape would mark
    /// every register item traced if the guard were dropped.
    #[test]
    fn a_bare_wildcard_covers_nothing(item in item()) {
        let identifier = compose("A1", &item, None);
        prop_assert!(
            !reference_covers(&identifier, "/*"),
            "a bare /* should cover no identifier, including {identifier:?}"
        );
    }

    /// A site wildcard covers sites on its item and not the bare item.
    ///
    /// `covers_site` requires the target to continue past the prefix with `@`,
    /// so `A1/item@*` names a site on that item. A rule that dropped the `@`
    /// requirement would claim the whole item.
    #[test]
    fn a_site_wildcard_covers_sites_and_not_the_item(
        area in area(),
        item in item(),
        symbol in item(),
        kind in site_kind(),
    ) {
        let wildcard = format!("{area}/{item}@*");
        let site = compose(&area, &item, Some((&symbol, &kind)));
        prop_assert!(
            reference_covers(&site, &wildcard),
            "{wildcard:?} should cover site {site:?}"
        );
        let plain = compose(&area, &item, None);
        prop_assert!(
            !reference_covers(&plain, &wildcard),
            "{wildcard:?} should not cover the bare item {plain:?}"
        );
    }

    /// A row of the declared width parses and keeps the cells it was given.
    ///
    /// The width check refuses a row that is not the shape its header
    /// declares. That refusal is only safe if the declared shape is accepted:
    /// a reader that rejected every row would make the binding rule vacuous
    /// while every rejection test still passed.
    #[test]
    fn a_trace_row_of_the_declared_width_parses(
        task in "[0-9]{1,2}\\.[0-9]{1,2}\\.[0-9]{1,2}",
        register in item(),
        obligations in prop::collection::vec("[0-9]{1,3}", 1..3),
    ) {
        let named: Vec<String> = obligations
            .iter()
            .map(|number| format!("CO-{number}"))
            .collect();
        let cell = named.join(", ");
        let row = format!("| {task} | {register} | -- | {cell} |");
        let parsed = prop_assert_ok!(parse_trace_row(&row));
        prop_assert_eq!(
            parsed.register_items, register,
            "row {:?} should keep its register cell", row
        );
        prop_assert_eq!(
            parsed.obligations, cell,
            "row {:?} should keep its obligations cell", row
        );
    }

    /// A row of any other width is refused, not read as a shifted neighbour.
    ///
    /// This is the direction that makes the width check worth having. After an
    /// omitted cell every later cell moves left, so a reader that accepted a
    /// short row would hand `Register items` the value of the column beside
    /// it — the exact misreading the check exists to prevent.
    #[test]
    fn a_trace_row_of_another_width_is_refused(
        cells in prop::collection::vec("[A-Za-z0-9-]{0,6}", 0..12),
    ) {
        prop_assume!(cells.len() != TRACE_COLUMNS);
        let row = format!("| {} |", cells.join(" | "));
        prop_assert!(
            parse_trace_row(&row).is_err(),
            "row {row:?} carries {} cells and should be refused",
            cells.len()
        );
    }
}
