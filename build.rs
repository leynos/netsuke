//! Build script for Netsuke.
//!
//! This script audits localization keys declared in
//! `src/localization/keys.rs` against the Fluent bundles in
//! `locales/*/messages.ftl`, failing the build if any declared key is missing
//! from a locale.
use clap::ArgMatches;
use std::{ffi::OsString, sync::Arc};

#[path = "src/cli/mod.rs"]
mod cli;

#[path = "src/cli_localization.rs"]
mod cli_localization;

#[path = "src/cli_l10n.rs"]
mod cli_l10n;

#[path = "src/host_pattern.rs"]
mod host_pattern;

#[path = "src/localization/mod.rs"]
mod localization;

#[path = "src/output_mode.rs"]
mod output_mode;

#[path = "src/theme.rs"]
mod theme;

mod build_l10n_audit;

use host_pattern::{HostPattern, HostPatternError};

type LocalizedParseFn = fn(
    Vec<OsString>,
    &Arc<dyn ortho_config::Localizer>,
) -> Result<(cli::Cli, ArgMatches), clap::Error>;

type ResolveThemeFn = fn(
    Option<theme::ThemePreference>,
    theme::ThemeContext,
    fn(&str) -> Option<String>,
) -> theme::ResolvedTheme;

type ThemeContextCtor = fn(
    Option<bool>,
    Option<cli::config::ColourPolicy>,
    output_mode::OutputMode,
) -> theme::ThemeContext;

/// Anchors all shared-module symbols so they remain linked when the build script is compiled
/// without tests.
const fn assert_symbols_linked() {
    const _: usize = std::mem::size_of::<HostPattern>();
    const _: fn(&[OsString]) -> Option<String> = cli::locale_hint_from_args;
    const _: fn(&[OsString]) -> Option<bool> = cli::diag_json_hint_from_args;
    const _: fn(&str) -> Option<bool> = cli_l10n::parse_bool_hint;
    const _: fn(&cli::Cli, &ArgMatches) -> bool = cli::resolve_merged_diag_json;
    const _: fn(&cli::Cli, &ArgMatches) -> ortho_config::OrthoResult<cli::Cli> =
        cli::merge_with_config;
    const _: LocalizedParseFn = cli::parse_with_localizer_from;
    const _: fn(&cli::Cli) -> cli::config::CliConfig = cli::Cli::config;
    const _: fn(&cli::Cli) -> bool = cli::Cli::resolved_diag_json;
    const _: fn(&cli::Cli) -> bool = cli::Cli::resolved_progress;
    const _: fn(&str) -> Result<HostPattern, HostPatternError> = HostPattern::parse;
    const _: fn(&HostPattern, host_pattern::HostCandidate<'_>) -> bool = HostPattern::matches;
    const _: fn(Option<bool>, Option<cli::config::ColourPolicy>) -> output_mode::OutputMode =
        output_mode::resolve;
    const _: fn(&cli::config::CliConfig) -> bool = cli::config::CliConfig::resolved_diag_json;
    const _: fn(&cli::config::CliConfig) -> bool = cli::config::CliConfig::resolved_progress;
    const _: ThemeContextCtor = theme::ThemeContext::new;
    const _: ResolveThemeFn = theme::resolve_theme;
}

/// Emits Cargo rerun directives for all inputs that affect the build output.
fn emit_rerun_directives() {
    println!("cargo:rerun-if-changed=src/cli/mod.rs");
    println!("cargo:rerun-if-changed=src/cli/parsing.rs");
    println!("cargo:rerun-if-changed=src/localization/keys.rs");
    println!("cargo:rerun-if-changed=locales/en-US/messages.ftl");
    println!("cargo:rerun-if-changed=locales/es-ES/messages.ftl");
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    assert_symbols_linked();
    emit_rerun_directives();
    build_l10n_audit::audit_localization_keys()?;
    Ok(())
}
