//! Build script: generate the CLI manual page into target/generated-man/<target>/<profile> for
//! release packaging.
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

fn extract_key_constants(path: &Path) -> Result<BTreeSet<String>, Box<dyn std::error::Error>> {
    let source = fs::read_to_string(path)?;
    let mut keys = BTreeSet::new();
    for line in source.lines() {
        let trimmed = line.trim();
        let Some((_, rest)) = trimmed.split_once("=> \"") else {
            continue;
        };
        let Some((key, _)) = rest.split_once('"') else {
            continue;
        };
        keys.insert(key.to_owned());
    }
    if keys.is_empty() {
        return Err(format!("no localization keys found in {}", path.display()).into());
    }
    Ok(keys)
}

fn extract_ftl_keys(path: &Path) -> Result<BTreeSet<String>, Box<dyn std::error::Error>> {
    let source = fs::read_to_string(path)?;
    let mut keys = BTreeSet::new();
    for line in source.lines() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('.') {
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

    if missing_en_us.is_empty() && missing_es_es.is_empty() {
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
