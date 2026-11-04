//! Layered encoding for the POSIX `printf %b` script transport.
//!
//! A `script` recipe may span many lines, while a Ninja `command` binding must
//! remain on one. The POSIX and Bash renderers carry the script through
//! `/bin/sh -e -c "printf %b '<operand>' | /bin/sh -e"`, so script text crosses
//! three parsers before the final interpreter reads it. Each function here
//! encodes exactly one layer, and [`escape_posix_script`] composes them from
//! the innermost outwards. Keeping the layers separate is what stops one
//! parser's protection from being consumed by the next; see the design
//! document's *POSIX script transport encoding* section.

/// Encode one POSIX script as the operand of the wrapper's `printf %b`.
///
/// This is the innermost layer. `printf %b` interprets backslash escapes, so
/// every literal backslash is doubled and every line feed becomes the two
/// characters `\n`. The doubling must happen before any outer layer adds
/// backslashes of its own; otherwise the outer shell consumes the protection
/// and `printf %b` reconstructs an escape from argument data.
fn encode_printf_operand(script: &str) -> String {
    let mut encoded = String::with_capacity(script.len());
    for character in script.chars() {
        match character {
            '\\' => encoded.push_str("\\\\"),
            '\n' => encoded.push_str("\\n"),
            _ => encoded.push(character),
        }
    }
    encoded
}

/// Encode the wrapper's `printf %b` operand as one POSIX single-quoted word.
///
/// This is the middle layer, consumed by the `/bin/sh` that runs
/// `printf %b '<operand>'`. A literal apostrophe closes the word, contributes
/// an escaped apostrophe, and reopens it, which is the standard single-quote
/// escape in POSIX shell.
fn encode_single_quoted_word(word: &str) -> String {
    word.replace('\'', r"'\''")
}

/// Encode one inner word for the wrapper's outer double-quoted string.
///
/// This is the outermost layer, consumed by the shell that receives
/// `/bin/sh -e -c "…"`. Inside double quotes only backslash, dollar, double
/// quote, and backtick are special, so each is prefixed with a backslash. The
/// backslash escape runs last, after the inner layers have added their own, so
/// a literal backslash becomes four backslashes in generated text and reaches
/// the final interpreter as one.
fn encode_double_quoted_word(word: &str) -> String {
    let mut encoded = String::with_capacity(word.len());
    for character in word.chars() {
        if matches!(character, '\\' | '$' | '"' | '`') {
            encoded.push('\\');
        }
        encoded.push(character);
    }
    encoded
}

/// Escape a POSIX script for the wrapper's single-quoted `printf %b` argument.
///
/// The wrapper is `/bin/sh -e -c "printf %b '<escaped>' | /bin/sh -e"`, so the
/// script crosses three parsers before the final interpreter reads it: the
/// outer shell's double quotes, the inner shell's single-quoted `%b` operand,
/// and `printf %b` itself. Each encoder above targets exactly one parser, and
/// they compose from the innermost outwards. Reordering them lets one layer
/// consume another's backslash protection and reconstruct shell syntax from
/// argument data.
pub(in crate::ninja_gen) fn escape_posix_script(script: &str) -> String {
    let operand = encode_printf_operand(script);
    let word = encode_single_quoted_word(&operand);
    encode_double_quoted_word(&word)
}
