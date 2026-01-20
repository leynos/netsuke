//! Build script for Netsuke.
//!
//! This script performs two main tasks:
//! - Generate the CLI manual page into `target/generated-man/<target>/<profile>` for release
//!   packaging.
//! - Audit localization keys declared in `src/localization/keys.rs` against the Fluent bundles
//!   in `locales/*/messages.ftl`, failing the build if any declared key is missing from a
//!   locale.
use clap::{ArgMatches, CommandFactory};
use clap_mangen::Man;
use std::{
    collections::BTreeSet,
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};
use time::{OffsetDateTime, format_description::well_known::Iso8601};

const FALLBACK_DATE: &str = "1970-01-01";

#[path = "src/cli.rs"]
mod cli;

#[path = "src/cli_localization.rs"]
mod cli_localization;

#[path = "src/cli_l10n.rs"]
mod cli_l10n;

#[path = "src/host_pattern.rs"]
mod host_pattern;

#[path = "src/localization/mod.rs"]
mod localization;

use host_pattern::{HostPattern, HostPatternError};

type LocalizedParseFn =
    fn(Vec<OsString>, &dyn ortho_config::Localizer) -> Result<(cli::Cli, ArgMatches), clap::Error>;

fn manual_date() -> String {
    let Ok(raw) = env::var("SOURCE_DATE_EPOCH") else {
        return FALLBACK_DATE.into();
    };

    let Ok(ts) = raw.parse::<i64>() else {
        println!(
            "cargo:warning=Invalid SOURCE_DATE_EPOCH '{raw}'; expected integer seconds since Unix epoch; falling back to {FALLBACK_DATE}"
        );
        return FALLBACK_DATE.into();
    };

    let Ok(dt) = OffsetDateTime::from_unix_timestamp(ts) else {
        println!(
            "cargo:warning=Invalid SOURCE_DATE_EPOCH '{raw}'; not a valid Unix timestamp; falling back to {FALLBACK_DATE}"
        );
        return FALLBACK_DATE.into();
    };

    dt.format(&Iso8601::DATE).unwrap_or_else(|_| {
        println!(
            "cargo:warning=Invalid SOURCE_DATE_EPOCH '{raw}'; formatting failed; falling back to {FALLBACK_DATE}"
        );
        FALLBACK_DATE.into()
    })
}

fn out_dir_for_target_profile() -> PathBuf {
    let target = env::var("TARGET").unwrap_or_else(|_| "unknown-target".into());
    let profile = env::var("PROFILE").unwrap_or_else(|_| "unknown-profile".into());
    PathBuf::from(format!("target/generated-man/{target}/{profile}"))
}

fn write_man_page(data: &[u8], dir: &Path, page_name: &str) -> std::io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let destination = dir.join(page_name);
    let tmp = dir.join(format!("{page_name}.tmp"));
    fs::write(&tmp, data)?;
    if destination.exists() {
        fs::remove_file(&destination)?;
    }
    fs::rename(&tmp, &destination)?;
    Ok(destination)
}

fn extract_define_keys_body(source: &str) -> Result<&str, Box<dyn std::error::Error>> {
    const MACRO_NAME: &str = "define_keys!";
    let Some(macro_pos) = source.find(MACRO_NAME) else {
        return Err("define_keys! macro not found in localization keys".into());
    };
    let after_macro = source
        .get(macro_pos + MACRO_NAME.len()..)
        .ok_or_else(|| "define_keys! macro start is out of range".to_owned())?;
    let Some(open_brace) = after_macro.find('{') else {
        return Err("define_keys! macro body is missing '{'".into());
    };
    let body_start = macro_pos + MACRO_NAME.len() + open_brace + 1;
    let remainder = source
        .get(body_start..)
        .ok_or_else(|| "define_keys! macro body is out of range".to_owned())?;
    let mut depth = 0usize;
    for (offset, ch) in remainder.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                if depth == 0 {
                    let body_end = body_start + offset;
                    return source
                        .get(body_start..body_end)
                        .ok_or_else(|| "define_keys! macro body slice invalid".into());
                }
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }
    Err("define_keys! macro body is missing '}'".into())
}

fn parse_string_literal(
    source: &str,
    start: usize,
) -> Result<(String, usize), Box<dyn std::error::Error>> {
    let bytes = source.as_bytes();
    if bytes.get(start) == Some(&b'"') {
        let mut value = String::new();
        let mut escaped = false;
        let remainder = source
            .get(start + 1..)
            .ok_or_else(|| "string literal start is out of range".to_owned())?;
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
        return Err("unterminated string literal in localization keys".into());
    }

    let mut idx = start;
    let mut has_byte_prefix = false;
    if bytes.get(idx) == Some(&b'b') {
        has_byte_prefix = true;
        idx += 1;
    }
    if bytes.get(idx) != Some(&b'r') {
        return Err("expected string literal after define_keys! =>".into());
    }
    if has_byte_prefix {
        return Err("byte string literals are not supported in localization keys".into());
    }
    idx += 1;
    let mut hash_count = 0usize;
    while bytes.get(idx) == Some(&b'#') {
        hash_count += 1;
        idx += 1;
    }
    if bytes.get(idx) != Some(&b'"') {
        return Err("raw string literal missing opening quote".into());
    }
    idx += 1;
    let content_start = idx;
    let mut pos = idx;
    while let Some(byte) = bytes.get(pos) {
        if *byte == b'"' {
            let mut matches = 0usize;
            while matches < hash_count && bytes.get(pos + 1 + matches) == Some(&b'#') {
                matches += 1;
            }
            if matches == hash_count {
                let content = source
                    .get(content_start..pos)
                    .ok_or_else(|| "raw string slice invalid".to_owned())?;
                let end = pos + 1 + hash_count;
                return Ok((content.to_owned(), end));
            }
        }
        pos += 1;
    }
    Err("unterminated raw string literal in localization keys".into())
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

fn parse_define_keys_body(body: &str) -> Result<BTreeSet<String>, Box<dyn std::error::Error>> {
    let bytes = body.as_bytes();
    let mut keys = BTreeSet::new();
    let mut idx = 0usize;
    while idx < bytes.len() {
        if is_line_comment(bytes, idx) {
            idx = skip_line_comment(bytes, idx + 2);
            continue;
        }
        if is_block_comment(bytes, idx) {
            idx = skip_block_comment(bytes, idx + 2);
            continue;
        }
        if bytes.get(idx) == Some(&b'=') && bytes.get(idx + 1) == Some(&b'>') {
            idx = skip_whitespace(bytes, idx + 2);
            if idx >= bytes.len() {
                break;
            }
            let (value, next) = parse_string_literal(body, idx)?;
            keys.insert(value);
            idx = next;
            continue;
        }
        idx += 1;
    }
    Ok(keys)
}

fn extract_key_constants(path: &Path) -> Result<BTreeSet<String>, Box<dyn std::error::Error>> {
    let source = fs::read_to_string(path)?;
    let body = extract_define_keys_body(&source)?;
    let keys = parse_define_keys_body(body)?;
    if keys.is_empty() {
        return Err(format!("no localization keys found in {}", path.display()).into());
    }
    Ok(keys)
}

fn should_skip_ftl_line(trimmed: &str) -> bool {
    trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('.')
}

fn extract_ftl_keys(path: &Path) -> Result<BTreeSet<String>, Box<dyn std::error::Error>> {
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

fn audit_localization_keys() -> Result<(), Box<dyn std::error::Error>> {
    let keys_path = Path::new("src/localization/keys.rs");
    let en_path = Path::new("locales/en-US/messages.ftl");
    let es_path = Path::new("locales/es-ES/messages.ftl");

    let declared = extract_key_constants(keys_path)?;
    let en_us_keys = extract_ftl_keys(en_path)?;
    let es_es_keys = extract_ftl_keys(es_path)?;

    let missing_en_us: Vec<_> = declared.difference(&en_us_keys).cloned().collect();
    let missing_es_es: Vec<_> = declared.difference(&es_es_keys).cloned().collect();
    let orphaned_en_us: Vec<_> = en_us_keys.difference(&declared).cloned().collect();
    let orphaned_es_es: Vec<_> = es_es_keys.difference(&declared).cloned().collect();

    if missing_en_us.is_empty()
        && missing_es_es.is_empty()
        && orphaned_en_us.is_empty()
        && orphaned_es_es.is_empty()
    {
        return Ok(());
    }

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
    Err(message.into())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Exercise CLI localization, config merge, and host pattern symbols so the
    // shared modules remain linked when the build script is compiled without
    // tests.
    const _: usize = std::mem::size_of::<HostPattern>();
    const _: fn(&[OsString]) -> Option<String> = cli::locale_hint_from_args;
    const _: fn(&cli::Cli, &ArgMatches) -> ortho_config::OrthoResult<cli::Cli> =
        cli::merge_with_config;
    const _: LocalizedParseFn = cli::parse_with_localizer_from;
    const _: fn(&str) -> Result<HostPattern, HostPatternError> = HostPattern::parse;
    const _: fn(&HostPattern, host_pattern::HostCandidate<'_>) -> bool = HostPattern::matches;

    // Regenerate the manual page when the CLI or metadata changes.
    println!("cargo:rerun-if-changed=src/cli.rs");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_VERSION");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_NAME");
    println!("cargo:rerun-if-env-changed=CARGO_BIN_NAME");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_DESCRIPTION");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_AUTHORS");
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
    println!("cargo:rerun-if-env-changed=TARGET");
    println!("cargo:rerun-if-env-changed=PROFILE");
    println!("cargo:rerun-if-changed=src/localization/keys.rs");
    println!("cargo:rerun-if-changed=locales/en-US/messages.ftl");
    println!("cargo:rerun-if-changed=locales/es-ES/messages.ftl");

    audit_localization_keys()?;

    // Packagers expect man pages under target/generated-man/<target>/<profile>.
    let out_dir = out_dir_for_target_profile();

    // The top-level page documents the entire command interface.
    let cmd = cli::Cli::command();
    let name = cmd
        .get_bin_name()
        .unwrap_or_else(|| cmd.get_name())
        .to_owned();
    let cargo_bin = env::var("CARGO_BIN_NAME")
        .or_else(|_| env::var("CARGO_PKG_NAME"))
        .unwrap_or_else(|_| name.clone());
    if name != cargo_bin {
        return Err(format!(
            "CLI name {name} differs from Cargo bin/package name {cargo_bin}; packaging expects {cargo_bin}.1"
        )
        .into());
    }
    let version = env::var("CARGO_PKG_VERSION").map_err(
        |_| "CARGO_PKG_VERSION must be set by Cargo; cannot render manual page without it.",
    )?;

    let man = Man::new(cmd)
        .section("1")
        .source(format!("{cargo_bin} {version}"))
        .date(manual_date());
    let mut buf = Vec::new();
    man.render(&mut buf)?;
    let page_name = format!("{cargo_bin}.1");
    write_man_page(&buf, &out_dir, &page_name)?;
    if let Some(extra_dir) = env::var_os("OUT_DIR") {
        let extra_dir_path = PathBuf::from(extra_dir);
        if let Err(err) = write_man_page(&buf, &extra_dir_path, &page_name) {
            println!(
                "cargo:warning=Failed to stage manual page in OUT_DIR ({}): {err}",
                extra_dir_path.display()
            );
        }
    }

    Ok(())
}
