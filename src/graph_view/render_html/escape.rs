//! HTML escaping helpers shared by the SVG, outline, and `<noscript>` writers.

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

pub(super) fn escape_text(input: &str) -> String {
    escape_html(input, false)
}

pub(super) fn escape_attr(input: &str) -> String {
    escape_html(input, true)
}
