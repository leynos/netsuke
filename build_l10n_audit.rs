//! Localization audit helpers for the build script.
//!
//! Parses the `define_keys!` macro in `src/localization/keys.rs` and compares
//! the declared keys with Fluent bundles to keep localized resources aligned
//! with the codebase.

use std::collections::BTreeSet;
use std::error::Error;
use std::fs;
use std::path::Path;

const DEFINE_KEYS_MACRO: &str = "define_keys!";
type KeySets = (BTreeSet<String>, BTreeSet<String>, BTreeSet<String>);

/// Represents the result of comparing declared keys against locale files.
struct AuditDifferences {
    missing_en_us: Vec<String>,
    missing_es_es: Vec<String>,
    orphaned_en_us: Vec<String>,
    orphaned_es_es: Vec<String>,
}

impl AuditDifferences {
    fn new(
        declared: &BTreeSet<String>,
        en_us_keys: &BTreeSet<String>,
        es_es_keys: &BTreeSet<String>,
    ) -> Self {
        Self {
            missing_en_us: declared.difference(en_us_keys).cloned().collect(),
            missing_es_es: declared.difference(es_es_keys).cloned().collect(),
            orphaned_en_us: en_us_keys.difference(declared).cloned().collect(),
            orphaned_es_es: es_es_keys.difference(declared).cloned().collect(),
        }
    }

    const fn has_issues(&self) -> bool {
        !self.missing_en_us.is_empty()
            || !self.missing_es_es.is_empty()
            || !self.orphaned_en_us.is_empty()
            || !self.orphaned_es_es.is_empty()
    }

    fn format_error_message(&self) -> String {
        build_audit_error_message(
            &self.missing_en_us,
            &self.missing_es_es,
            &self.orphaned_en_us,
            &self.orphaned_es_es,
        )
    }
}

fn load_key_sets(
    keys_path: &Path,
    en_path: &Path,
    es_path: &Path,
) -> Result<KeySets, Box<dyn Error>> {
    let declared = extract_key_constants(keys_path)?;
    let en_us_keys = extract_ftl_keys(en_path)?;
    let es_es_keys = extract_ftl_keys(es_path)?;
    Ok((declared, en_us_keys, es_es_keys))
}

fn compute_audit_differences(
    declared: &BTreeSet<String>,
    en_us_keys: &BTreeSet<String>,
    es_es_keys: &BTreeSet<String>,
) -> AuditDifferences {
    AuditDifferences::new(declared, en_us_keys, es_es_keys)
}

fn build_audit_error_message(
    missing_en_us: &[String],
    missing_es_es: &[String],
    orphaned_en_us: &[String],
    orphaned_es_es: &[String],
) -> String {
    let mut message = String::from("localization key audit failed:");
    if !missing_en_us.is_empty() {
        message.push_str("\n- missing in en-US: ");
        message.push_str(&missing_en_us.join(", "));
    }
    if !missing_es_es.is_empty() {
        message.push_str("\n- missing in es-ES: ");
        message.push_str(&missing_es_es.join(", "));
    }
    if !orphaned_en_us.is_empty() {
        message.push_str("\n- orphaned in en-US: ");
        message.push_str(&orphaned_en_us.join(", "));
    }
    if !orphaned_es_es.is_empty() {
        message.push_str("\n- orphaned in es-ES: ");
        message.push_str(&orphaned_es_es.join(", "));
    }
    message
}

pub(super) fn audit_localization_keys() -> Result<(), Box<dyn Error>> {
    let keys_path = Path::new("src/localization/keys.rs");
    let en_path = Path::new("locales/en-US/messages.ftl");
    let es_path = Path::new("locales/es-ES/messages.ftl");

    let (declared, en_us_keys, es_es_keys) = load_key_sets(keys_path, en_path, es_path)?;
    let results = compute_audit_differences(&declared, &en_us_keys, &es_es_keys);
    if results.has_issues() {
        Err(results.format_error_message().into())
    } else {
        Ok(())
    }
}

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
fn extract_key_constants(path: &Path) -> Result<BTreeSet<String>, Box<dyn Error>> {
    let source = fs::read_to_string(path)?;
    let body = extract_define_keys_body(&source)?;
    let keys = parse_define_keys_body(body)?;
    if keys.is_empty() {
        return Err(format!("no localization keys found in {}", path.display()).into());
    }
    Ok(keys)
}

fn extract_define_keys_body(source: &str) -> Result<&str, Box<dyn Error>> {
    let Some(macro_pos) = source.find(DEFINE_KEYS_MACRO) else {
        return Err("define_keys! macro not found in localization keys".into());
    };
    let after_macro = source
        .get(macro_pos + DEFINE_KEYS_MACRO.len()..)
        .ok_or_else(|| "define_keys! macro start is out of range".to_owned())?;
    let Some(open_brace) = after_macro.find('{') else {
        return Err("define_keys! macro body is missing '{'".into());
    };
    let body_start = macro_pos + DEFINE_KEYS_MACRO.len() + open_brace + 1;
    let remainder = source
        .get(body_start..)
        .ok_or_else(|| "define_keys! macro body is out of range".to_owned())?;
    let body_len = find_matching_brace(remainder)?;
    let body_end = body_start + body_len;
    source
        .get(body_start..body_end)
        .ok_or_else(|| "define_keys! macro body slice invalid".into())
}

fn find_matching_brace(source: &str) -> Result<usize, Box<dyn Error>> {
    let mut depth = 0usize;
    for (offset, ch) in source.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                if depth == 0 {
                    return Ok(offset);
                }
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }
    Err("define_keys! macro body is missing '}'".into())
}

fn parse_string_literal(source: &str, start: usize) -> Result<(String, usize), Box<dyn Error>> {
    if source.as_bytes().get(start) == Some(&b'"') {
        return parse_regular_string_literal(source, start);
    }
    parse_raw_string_literal(source, start)
}

fn parse_regular_string_literal(
    source: &str,
    start: usize,
) -> Result<(String, usize), Box<dyn Error>> {
    let remainder = source
        .get(start + 1..)
        .ok_or_else(|| "string literal start is out of range".to_owned())?;
    let mut value = String::new();
    let mut escaped = false;
    for (offset, ch) in remainder.char_indices() {
        if escaped {
            value.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '"' => {
                let end = start + 1 + offset + 1;
                return Ok((value, end));
            }
            _ => value.push(ch),
        }
    }
    Err("unterminated string literal in localization keys".into())
}

fn parse_raw_string_literal(source: &str, start: usize) -> Result<(String, usize), Box<dyn Error>> {
    let bytes = source.as_bytes();
    let (mut idx, has_byte_prefix) = parse_raw_prefix(bytes, start)?;
    if has_byte_prefix {
        return Err("byte string literals are not supported in localization keys".into());
    }
    let hash_count = count_hashes(bytes, &mut idx);
    if bytes.get(idx) != Some(&b'"') {
        return Err("raw string literal missing opening quote".into());
    }
    idx += 1;
    let content_start = idx;
    let end = find_raw_string_end(bytes, idx, hash_count)
        .ok_or_else(|| "unterminated raw string literal in localization keys".to_owned())?;
    let content_end = end - 1 - hash_count;
    let content = source
        .get(content_start..content_end)
        .ok_or_else(|| "raw string slice invalid".to_owned())?;
    Ok((content.to_owned(), end))
}

fn parse_raw_prefix(bytes: &[u8], start: usize) -> Result<(usize, bool), Box<dyn Error>> {
    let mut idx = start;
    let has_byte_prefix = bytes.get(idx) == Some(&b'b');
    if has_byte_prefix {
        idx += 1;
    }
    if bytes.get(idx) != Some(&b'r') {
        return Err("expected string literal after define_keys! =>".into());
    }
    Ok((idx + 1, has_byte_prefix))
}

fn count_hashes(bytes: &[u8], idx: &mut usize) -> usize {
    let mut count = 0usize;
    while bytes.get(*idx) == Some(&b'#') {
        count += 1;
        *idx += 1;
    }
    count
}

fn find_raw_string_end(bytes: &[u8], mut pos: usize, hash_count: usize) -> Option<usize> {
    while let Some(byte) = bytes.get(pos) {
        if *byte == b'"' && raw_hashes_match(bytes, pos + 1, hash_count) {
            return Some(pos + 1 + hash_count);
        }
        pos += 1;
    }
    None
}

fn raw_hashes_match(bytes: &[u8], start: usize, count: usize) -> bool {
    (0..count).all(|idx| bytes.get(start + idx) == Some(&b'#'))
}

fn is_line_comment(bytes: &[u8], idx: usize) -> bool {
    bytes.get(idx) == Some(&b'/') && bytes.get(idx + 1) == Some(&b'/')
}

fn is_block_comment(bytes: &[u8], idx: usize) -> bool {
    bytes.get(idx) == Some(&b'/') && bytes.get(idx + 1) == Some(&b'*')
}

fn skip_line_comment(bytes: &[u8], mut idx: usize) -> usize {
    while let Some(byte) = bytes.get(idx) {
        idx += 1;
        if *byte == b'\n' {
            break;
        }
    }
    idx
}

fn skip_block_comment(bytes: &[u8], mut idx: usize) -> usize {
    while idx + 1 < bytes.len() {
        if bytes.get(idx) == Some(&b'*') && bytes.get(idx + 1) == Some(&b'/') {
            return idx + 2;
        }
        idx += 1;
    }
    bytes.len()
}

fn skip_whitespace(bytes: &[u8], mut idx: usize) -> usize {
    while let Some(byte) = bytes.get(idx) {
        if byte.is_ascii_whitespace() {
            idx += 1;
        } else {
            break;
        }
    }
    idx
}

/// Attempts to parse a key-value pair starting at the given index.
/// Returns the extracted key and the next index to continue parsing.
fn try_parse_key_at_arrow(
    body: &str,
    bytes: &[u8],
    idx: usize,
) -> Result<Option<(String, usize)>, Box<dyn Error>> {
    if bytes.get(idx) != Some(&b'=') || bytes.get(idx + 1) != Some(&b'>') {
        return Ok(None);
    }

    let next_idx = skip_whitespace(bytes, idx + 2);
    if next_idx >= bytes.len() {
        return Ok(None);
    }

    let (value, next) = parse_string_literal(body, next_idx)?;
    Ok(Some((value, next)))
}

fn process_token_at(
    body: &str,
    bytes: &[u8],
    idx: usize,
) -> Result<Option<(String, usize)>, Box<dyn Error>> {
    if idx >= bytes.len() {
        return Ok(None);
    }
    if is_line_comment(bytes, idx) {
        return Ok(Some((String::new(), skip_line_comment(bytes, idx + 2))));
    }
    if is_block_comment(bytes, idx) {
        return Ok(Some((String::new(), skip_block_comment(bytes, idx + 2))));
    }
    if let Some((key, next)) = try_parse_key_at_arrow(body, bytes, idx)? {
        return Ok(Some((key, next)));
    }
    Ok(Some((String::new(), idx + 1)))
}

fn parse_define_keys_body(body: &str) -> Result<BTreeSet<String>, Box<dyn Error>> {
    let bytes = body.as_bytes();
    let mut keys = BTreeSet::new();
    let mut idx = 0usize;
    while idx < bytes.len() {
        let Some((value, next)) = process_token_at(body, bytes, idx)? else {
            break;
        };
        if !value.is_empty() {
            keys.insert(value);
        }
        idx = next;
    }
    Ok(keys)
}

fn should_skip_ftl_line(trimmed: &str) -> bool {
    trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('.')
}

/// Extract Fluent message identifiers from a `.ftl` bundle.
///
/// This parser expects simple message declarations of the form `id = ...` and
/// skips blank lines, comments (starting with `#`), and attributes (starting
/// with `.`). Term identifiers (those starting with `-`) are ignored by design
/// because Netsuke only references message IDs in code.
///
/// # Errors
///
/// Returns an error if no keys are found in the bundle.
fn extract_ftl_keys(path: &Path) -> Result<BTreeSet<String>, Box<dyn Error>> {
    let source = fs::read_to_string(path)?;
    let mut keys = BTreeSet::new();
    for line in source.lines() {
        let trimmed = line.trim_start();
        if should_skip_ftl_line(trimmed) {
            continue;
        }
        let Some((id_raw, _)) = trimmed.split_once('=') else {
            continue;
        };
        let id = id_raw.trim();
        if id.is_empty() || id.starts_with('-') {
            continue;
        }
        keys.insert(id.to_owned());
    }
    if keys.is_empty() {
        return Err(format!("no Fluent keys found in {}", path.display()).into());
    }
    Ok(keys)
}
