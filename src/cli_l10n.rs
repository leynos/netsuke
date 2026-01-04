//! CLI localization helpers.
//!
//! This module keeps clap localization logic separate from the core CLI
//! definitions.

use clap::Command;
use ortho_config::{LocalizationArgs, Localizer};
use std::ffi::OsString;

fn usage_body(usage: &str) -> &str {
    usage.strip_prefix("Usage: ").unwrap_or(usage)
}

pub(crate) fn localize_command(mut command: Command, localizer: &dyn Localizer) -> Command {
    let rendered_usage = command.render_usage().to_string();
    let fallback_usage = usage_body(&rendered_usage).to_owned();
    let mut args = LocalizationArgs::default();
    args.insert("binary", command.get_name().to_owned().into());
    args.insert("usage", fallback_usage.clone().into());
    let usage = localizer.message("cli.usage", Some(&args), &fallback_usage);
    command = command.override_usage(usage);

    if let Some(about) = command.get_about().map(ToString::to_string) {
        let localized_text = localizer.message("cli.about", None, &about);
        command = command.about(localized_text);
    } else if let Some(message) = localizer.lookup("cli.about", None) {
        command = command.about(message);
    }

    if let Some(long_about) = command.get_long_about().map(ToString::to_string) {
        let localized_text = localizer.message("cli.long_about", None, &long_about);
        command = command.long_about(localized_text);
    } else if let Some(message) = localizer.lookup("cli.long_about", None) {
        command = command.long_about(message);
    }

    localize_subcommands(&mut command, localizer);

    command
}

fn localize_subcommands(command: &mut Command, localizer: &dyn Localizer) {
    for subcommand in command.get_subcommands_mut() {
        let name = subcommand.get_name().to_owned();
        let mut updated = std::mem::take(subcommand);
        let about_key = format!("cli.subcommand.{name}.about");
        if let Some(about) = updated.get_about().map(ToString::to_string) {
            let message = localizer.message(&about_key, None, &about);
            updated = updated.about(message);
        } else if let Some(message) = localizer.lookup(&about_key, None) {
            updated = updated.about(message);
        }

        let long_key = format!("cli.subcommand.{name}.long_about");
        if let Some(long_about) = updated.get_long_about().map(ToString::to_string) {
            let message = localizer.message(&long_key, None, &long_about);
            updated = updated.long_about(message);
        } else if let Some(message) = localizer.lookup(&long_key, None) {
            updated = updated.long_about(message);
        }

        *subcommand = updated;
    }
}

/// Inspect raw arguments and extract the `--locale` value when present.
///
/// When multiple `--locale` flags are provided, the last one is used.
#[must_use]
pub fn locale_hint_from_args(args: &[OsString]) -> Option<String> {
    let mut hint = None;
    let mut iter = args.iter().peekable();
    while let Some(arg) = iter.next() {
        let text = arg.to_string_lossy();
        if text == "--" {
            break;
        }
        if text == "--locale" {
            let Some(next) = iter.peek() else {
                break;
            };
            let next_text = next.to_string_lossy();
            if next_text == "--" {
                break;
            }
            hint = Some(next_text.into_owned());
            iter.next();
            continue;
        }
        if let Some(value) = text.strip_prefix("--locale=") {
            hint = Some(value.to_owned());
        }
    }
    hint
}
