//! Extraction of Fluent message identifiers declared in Rust source.
//!
//! Parses the `define_keys!` macro in `src/localization/keys.rs` so the build
//! audit can compare the keys the code references against the keys each
//! catalogue provides.

#[path = "scanner.rs"]
mod scanner;

use scanner::{ByteIndex, DefineKeysParser};
use std::collections::BTreeSet;
use std::error::Error;

/// The macro invocation the audit scans for.
const DEFINE_KEYS_MACRO: &str = "define_keys!";

/// Extracts localization key values from `keys.rs`.
///
/// Parses the `define_keys!` macro invocation to extract Fluent key identifiers.
/// Expects entries of the form: `CONST_NAME => "fluent-key-id",` within the
/// macro body.
///
/// Implementation note: uses `extract_define_keys_body` to locate the macro
/// body and `parse_define_keys_body` to read values from `=> "..."` patterns.
///
/// # Errors
///
/// Returns an error if the macro cannot be parsed or no keys are found.
pub(super) fn extract_key_constants(source: &str) -> Result<BTreeSet<String>, Box<dyn Error>> {
    let body = extract_define_keys_body(source)?;
    let keys = parse_define_keys_body(body)?;
    if keys.is_empty() {
        return Err("no localization keys found in the localization key source".into());
    }
    Ok(keys)
}

/// Slice out the text between the macro's `{` and its matching `}`.
///
/// # Errors
///
/// Returns an error if the macro or its closing brace cannot be found.
fn extract_define_keys_body(source: &str) -> Result<&str, Box<dyn Error>> {
    // Scanned rather than searched: `define_keys!` named in a doc comment or
    // quoted in a string is not the invocation, and reading from there would
    // take the wrong text as the macro body.
    let Some(macro_pos) = DefineKeysParser::new(source).find_in_source(DEFINE_KEYS_MACRO) else {
        return Err("define_keys! macro not found in localization keys".into());
    };
    // Trivia may sit between the macro name and its delimiter, and a brace
    // inside it is not the delimiter: `define_keys! /* { */ { … }` is valid
    // Rust.
    let parser = DefineKeysParser::new(source);
    let after_name = macro_pos
        .checked_add(DEFINE_KEYS_MACRO.len())
        .ok_or_else(|| "define_keys! macro start is out of range".to_owned())?;
    let Some(body_start) = parser.body_start_after(ByteIndex::from_offset(after_name)) else {
        return Err("define_keys! macro body is missing '{'".into());
    };
    let remainder = source
        .get(body_start..)
        .ok_or_else(|| "define_keys! macro body is out of range".to_owned())?;
    let body_len = find_matching_brace(remainder)?;
    let body_end = body_start + body_len;
    source
        .get(body_start..body_end)
        .ok_or_else(|| "define_keys! macro body slice invalid".into())
}

/// Offset of the `}` closing a body that begins at the start of `source`.
///
/// Braces inside comments and string literals do not nest, so the scan skips
/// both. A doc comment mentioning `}` or a key whose value contains one would
/// otherwise end the body early and silently truncate the declared key set.
fn find_matching_brace(source: &str) -> Result<usize, Box<dyn Error>> {
    DefineKeysParser::new(source).find_body_end()
}

/// Collect the `"..."` values from `=>` entries in `body`.
///
/// # Errors
///
/// Returns an error if an entry's string literal is malformed.
fn parse_define_keys_body(body: &str) -> Result<BTreeSet<String>, Box<dyn Error>> {
    let parser = DefineKeysParser::new(body);
    let mut keys = BTreeSet::new();
    let mut index = ByteIndex::START;
    while !parser.is_exhausted(index) {
        let Some((value, next)) = parser.process_token_at(index)? else {
            break;
        };
        if !value.is_empty() {
            keys.insert(value);
        }
        index = next;
    }
    Ok(keys)
}
