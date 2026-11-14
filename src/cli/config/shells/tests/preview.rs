//! Bounding and escaping rules for text rendered into diagnostics.
//!
//! These rules protect a terminal from operator-supplied text that is long or
//! contains control sequences. The startup suite proves the declaring path
//! reaches a real diagnostic through a real configuration load, but a
//! filesystem cannot carry every input these rules exist for: Windows refuses a
//! directory name containing a control character. Stating the rules directly
//! here keeps them exercised on every platform, and keeps the integration case
//! to the input a filesystem can hold everywhere.

use crate::cli::config::shells::preview::{bounded, preview};

/// Render control characters so a diagnostic stays on one line.
#[test]
fn bounded_escapes_control_characters() {
    assert_eq!(bounded("odd\u{7}directory"), "odd\\u{7}directory");
    assert_eq!(bounded("first\nsecond"), "first\\nsecond");
}

/// Mark a preview that does not show the whole value.
#[test]
fn bounded_marks_a_truncated_value() {
    let overlong = "x".repeat(90);
    let rendered = bounded(&overlong);
    let prefix = rendered
        .strip_suffix('…')
        .expect("a truncated preview ends with an ellipsis");
    assert_eq!(prefix, "x".repeat(64).as_str());
}

/// Leave a value that fits inside the bound whole and unmarked.
#[test]
fn bounded_keeps_a_value_at_the_bound() {
    let exact = "x".repeat(64);
    assert_eq!(bounded(&exact), exact);
    let shorter = exact.strip_suffix('x').expect("the fixture is all x");
    assert_eq!(bounded(shorter), shorter);
}

/// Quote a preview so it is distinguishable from the surrounding message.
#[test]
fn preview_is_quoted() {
    assert_eq!(preview("odd\u{7}"), "\"odd\\u{7}\"");
}
