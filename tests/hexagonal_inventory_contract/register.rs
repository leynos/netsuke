//! Check register identifiers against the inventory's area and site grammar.
//!
//! The grammar lives apart from the Markdown parsers because it is a property
//! of the identifiers themselves, not of the document that carries them:
//! `markdown` reads identifiers out of tables and bullets, and `validation`
//! compares them, but only this module decides whether one is well formed.

/// Check whether a register identifier follows the area and site grammar.
///
/// A well-formed identifier is `AREA/ITEM` or `AREA/ITEM@SYMBOL#KIND`, where
/// `AREA` is one of the closed set `A1`–`A7`, `ITEM` carries no `#`, and `KIND`
/// is a supported site kind with an optional numeric suffix.
pub(super) fn is_well_formed_register_id(identifier: &str) -> bool {
    let Some((area, body)) = identifier.split_once('/') else {
        return false;
    };
    has_valid_area_and_body(area, body) && has_valid_register_body(body)
}

/// Check whether the register body uses either a type or a site identifier.
fn has_valid_register_body(body: &str) -> bool {
    let Some((item, site)) = body.split_once('@') else {
        return !body.contains('#');
    };
    has_valid_item(item) && has_valid_site(site)
}

/// Check whether the item before the site marker is non-empty and unambiguous.
fn has_valid_item(item: &str) -> bool {
    !item.is_empty() && !item.contains('#')
}

/// Check whether a site has a symbol and a supported kind.
fn has_valid_site(site: &str) -> bool {
    let Some((symbol, kind)) = site.split_once('#') else {
        return false;
    };
    !symbol.is_empty()
        && !has_site_delimiter(symbol)
        && !has_site_delimiter(kind)
        && has_supported_site_kind(kind)
}

/// Reject separators that would make a site split ambiguous.
fn has_site_delimiter(value: &str) -> bool {
    value
        .chars()
        .any(|character| matches!(character, '@' | '#'))
}

/// Check the closed set of site kinds and their numbered variants.
fn has_supported_site_kind(kind: &str) -> bool {
    ["constructor", "mutator", "consumer", "api-reachable"]
        .iter()
        .any(|known_kind| matches_site_kind(kind, known_kind))
}

/// Match a site kind or its numbered suffix.
fn matches_site_kind(kind: &str, known_kind: &str) -> bool {
    kind == known_kind
        || kind
            .strip_prefix(known_kind)
            .and_then(|suffix| suffix.strip_prefix('-'))
            .is_some_and(is_ascii_number)
}

/// Check that a numbered site-kind suffix contains only decimal digits.
fn is_ascii_number(number: &str) -> bool {
    !number.is_empty() && number.chars().all(|character| character.is_ascii_digit())
}

/// Check the register area's closed set and its non-empty, whitespace-free body.
fn has_valid_area_and_body(area: &str, body: &str) -> bool {
    matches!(area, "A1" | "A2" | "A3" | "A4" | "A5" | "A6" | "A7")
        && !body.is_empty()
        && !body.chars().any(char::is_whitespace)
}
