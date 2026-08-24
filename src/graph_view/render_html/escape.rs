//! HTML escaping helpers shared by the SVG, outline, and `<noscript>` writers.

/// Escape `input` for HTML, escaping double-quote and single-quote characters
/// as HTML entities when `attr` is `true`.
fn escape_html(input: &str, attr: bool) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if attr => out.push_str("&quot;"),
            '\'' if attr => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

/// Escape `input` for HTML text content.
pub(super) fn escape_text(input: &str) -> String {
    escape_html(input, false)
}

/// Escape `input` for a double-quoted HTML attribute value.
pub(super) fn escape_attr(input: &str) -> String {
    escape_html(input, true)
}
