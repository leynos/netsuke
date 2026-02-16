//! Fluent localization test utilities.
//!
//! Provides helpers for normalizing Fluent output in test assertions.

/// Remove Fluent bidi isolate characters used around placeables.
///
/// Fluent inserts these markers (`\u{2068}` and `\u{2069}`) to preserve
/// directionality when interpolating values. They are invisible to users but
/// can make plain substring assertions fail.
///
/// # Examples
///
/// ```
/// use test_support::fluent::normalize_fluent_isolates;
///
/// let raw = "Stage \u{2068}2\u{2069}/\u{2068}6\u{2069}";
/// assert_eq!(normalize_fluent_isolates(raw), "Stage 2/6");
/// ```
#[must_use]
pub fn normalize_fluent_isolates(text: &str) -> String {
    text.chars()
        .filter(|ch| *ch != '\u{2068}' && *ch != '\u{2069}')
        .collect()
}
