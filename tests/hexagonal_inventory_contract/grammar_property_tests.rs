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

/// Report whether an identifier is exactly the canonical rendering of its parts.
///
/// An accepted string must re-compose from the parts its own split produces.
/// This catches a reader that accepts a longer form than it documents, such as
/// trailing text on a kind, which a prefix check alone would allow.
fn is_canonical(identifier: &str) -> bool {
    let Some((area, body)) = identifier.split_once('/') else {
        return false;
    };
    let Some((item, site)) = body.split_once('@') else {
        return compose(area, body, None) == identifier;
    };
    let Some((symbol, kind)) = site.split_once('#') else {
        return false;
    };
    compose(area, item, Some((symbol, kind))) == identifier
}

proptest! {
    /// An accepted identifier names a permitted area and is canonical.
    ///
    /// The area half is what makes this a guard rather than a tautology. An
    /// identifier built from the generator's own parts re-composes by
    /// construction, so `is_canonical` alone would pass a reader that accepted
    /// any area at all — it did, until the closed set was asserted here. The
    /// canonical half then rejects a reader accepting a longer form than it
    /// documents, such as trailing text on a kind.
    #[test]
    fn accepted_identifiers_name_a_permitted_area_and_are_canonical(
        area in area(),
        item in item(),
        site in prop::option::of((item(), site_kind())),
    ) {
        let identifier = match &site {
            Some((symbol, kind)) => compose(&area, &item, Some((symbol, kind))),
            None => compose(&area, &item, None),
        };
        if is_well_formed_register_id(&identifier) {
            prop_assert!(
                AREAS.contains(&area.as_str()),
                "accepted {identifier:?} names area {area:?}, which is not permitted"
            );
            prop_assert!(
                is_canonical(&identifier),
                "accepted {identifier:?} should re-compose from its parts"
            );
        }
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
