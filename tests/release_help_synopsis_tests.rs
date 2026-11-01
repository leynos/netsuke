//! Behavioural coverage for the release-help man-page SYNOPSIS.
//!
//! `scripts/generate-release-help.sh` is the release boundary that turns the
//! metadata `cargo-orthohelp` reads out of Netsuke into the published manual
//! page. `cargo-orthohelp` prints `[missing: <id>]` when the root
//! `synopsis_id` names an undeclared catalogue key, and lists the documented
//! options when the identifier is absent. Driving the real script with a fake
//! generator that models both cases keeps the generator-facing contract under
//! test: the metadata is the value Netsuke actually serialises, so restoring
//! the derived `netsuke-usage` identifier (#959) fails here as well as in the
//! metadata unit tests.

#![cfg(unix)]

use anyhow::{Context, Result, ensure};
use netsuke::cli::ReleaseHelpCli;
use ortho_config::docs::OrthoConfigDocs;
use serde_json::Value;
use std::{
    ffi::OsString,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Output},
};

const SCRIPT_PATH: &str = "scripts/generate-release-help.sh";
const TARGET: &str = "x86_64-unknown-linux-gnu";

const FAKE_GENERATOR: &str = r#"#!/usr/bin/env python3
"""Model cargo-orthohelp's synopsis resolution for release-help tests."""

import json
import os
import sys
from pathlib import Path


def option_value(arguments, name):
    for index, argument in enumerate(arguments):
        if argument == name:
            return arguments[index + 1]
    return ""


def parser_only_selectors(fields):
    selectors = []
    for field in fields:
        cli = field.get("cli")
        if not cli or field.get("env") is not None or field.get("file") is not None:
            continue
        long = cli.get("long")
        if not long:
            continue
        short = cli.get("short")
        selectors.append(f"-{short}/--{long}" if short else f"--{long}")
    return selectors


def option_entries(fields):
    entries = []
    for field in fields:
        cli = field.get("cli")
        if not cli:
            continue
        long = cli.get("long")
        if not long:
            continue
        short = cli.get("short")
        entry = [f"-{short}/--{long}" if short else f"--{long}"]
        possible_values = cli.get("possible_values") or []
        if possible_values:
            entry.append("Possible values: " + ", ".join(possible_values))
        entries.append(entry)
    return entries


def main():
    arguments = sys.argv[1:]
    if option_value(arguments, "--format") != "man":
        return 0

    metadata = json.loads(
        Path(os.environ["RELEASE_HELP_SYNOPSIS_METADATA"]).read_text(encoding="utf-8")
    )
    synopsis_id = metadata.get("synopsis_id")
    if synopsis_id:
        synopsis = f"[missing: {synopsis_id}]"
    else:
        synopsis = " ".join(parser_only_selectors(metadata.get("fields", [])))

    man_dir = Path(option_value(arguments, "--out-dir")) / "man" / "man1"
    man_dir.mkdir(parents=True, exist_ok=True)
    man_page = "\n".join(
        [
            '.TH NETSUKE 1 "1970-01-01"',
            ".SH NAME",
            "netsuke \\- dependency-aware build orchestration",
            ".SH SYNOPSIS",
            ".B netsuke",
            synopsis,
            "",
            ".SH OPTIONS",
        ]
    )
    # The script verifies the rendered surface, so this model has to render the
    # documented options too; only the SYNOPSIS resolution is under test here.
    for flag, *details in option_entries(metadata.get("fields", [])):
        man_page += "\n.TP\n.B " + flag
        for detail in details:
            man_page += "\n" + detail
    (man_dir / "netsuke.1").write_text(man_page, encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
"#;

/// Install the fake `cargo-orthohelp` generator into `bin_dir`.
///
/// # Errors
///
/// Returns an error when the file cannot be written, inspected, or marked
/// executable.
fn install_fake_generator(bin_dir: &Path) -> Result<()> {
    let path = bin_dir.join("cargo-orthohelp");
    fs::create_dir_all(bin_dir).context("create fake generator directory")?;
    fs::write(&path, FAKE_GENERATOR)
        .with_context(|| format!("write fake generator {}", path.display()))?;
    let mut permissions = fs::metadata(&path)
        .with_context(|| format!("read fake generator metadata {}", path.display()))?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions)
        .with_context(|| format!("mark fake generator executable {}", path.display()))
}

/// Prepend `bin_dir` to the `PATH` inherited by the test process.
///
/// # Errors
///
/// Returns an error when the platform-specific `PATH` cannot be constructed.
#[expect(
    clippy::disallowed_methods,
    reason = "locating the interpreter and tool directories this test inherits; there is no seam to inject and no process state to isolate"
)]
fn path_with_fake_generator(bin_dir: &Path) -> Result<OsString> {
    let inherited = std::env::var_os("PATH").unwrap_or_default();
    let entries = std::iter::once(bin_dir.to_path_buf()).chain(std::env::split_paths(&inherited));
    std::env::join_paths(entries).context("construct PATH with the fake generator first")
}

/// Run the release-help script against the fake generator.
///
/// # Errors
///
/// Returns an error when the script cannot be started.
fn run_release_help(bin_dir: &Path, metadata_path: &Path, out_dir: &Path) -> Result<Output> {
    Command::new("bash")
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(SCRIPT_PATH))
        .arg(TARGET)
        .arg("netsuke")
        .arg(out_dir)
        .arg("Netsuke")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("PATH", path_with_fake_generator(bin_dir)?)
        .env("RELEASE_HELP_SYNOPSIS_METADATA", metadata_path)
        .env_remove("GITHUB_RUN_ID")
        .env_remove("GITHUB_RUN_ATTEMPT")
        .env_remove("RELEASE_HELP_BUILD_ID")
        .env_remove("RELEASE_HELP_CORRELATION_ID")
        .env_remove("SOURCE_DATE_EPOCH")
        .output()
        .context("run release help script against the fake generator")
}

/// Write serialised release-help metadata for the fake generator to consume.
///
/// # Errors
///
/// Returns an error when the metadata cannot be serialised or written.
fn write_metadata(path: &Path, metadata: &Value) -> Result<()> {
    let bytes = serde_json::to_vec(metadata).context("serialise release-help metadata")?;
    fs::write(path, bytes)
        .with_context(|| format!("write release-help metadata {}", path.display()))
}

/// Return the SYNOPSIS section of a generated manual page.
fn synopsis_section(man_page: &str) -> Option<&str> {
    let rest = man_page.split_once(".SH SYNOPSIS\n")?.1;
    Some(rest.split_once(".SH ").map_or(rest, |(section, _)| section))
}

/// Verify the generated manual page uses the option-listing SYNOPSIS fallback.
///
/// # Errors
///
/// Returns an error when the metadata, script run, or artefact assertions
/// cannot be satisfied.
#[test]
fn man_page_synopsis_uses_the_option_listing_fallback() -> Result<()> {
    let temp = tempfile::tempdir().context("create release-help synopsis tempdir")?;
    let bin_dir = temp.path().join("bin");
    install_fake_generator(&bin_dir)?;

    let metadata = serde_json::to_value(ReleaseHelpCli::get_doc_metadata())
        .context("serialise release-help metadata to JSON")?;
    let synopsis_id = metadata.get("synopsis_id").cloned().unwrap_or(Value::Null);
    ensure!(
        synopsis_id.is_null(),
        "release help must clear the root synopsis_id so the generator renders \
         its option-listing fallback, got {synopsis_id}"
    );
    let metadata_path = temp.path().join("release-help-metadata.json");
    write_metadata(&metadata_path, &metadata)?;

    let out_dir = temp.path().join("out");
    let output = run_release_help(&bin_dir, &metadata_path, &out_dir)?;
    ensure!(
        output.status.success(),
        "release-help script failed: {output:?}"
    );

    let man_page = fs::read_to_string(out_dir.join("man/man1/netsuke.1"))
        .context("read the generated manual page")?;
    ensure!(
        !man_page.contains("[missing:"),
        "the manual page must not advertise an unresolved synopsis identifier: \
         {man_page}"
    );
    let synopsis = synopsis_section(&man_page)
        .context("the generated manual page should declare a SYNOPSIS section")?;
    for selector in ["-C/--directory", "--config"] {
        ensure!(
            synopsis.contains(selector),
            "the SYNOPSIS fallback should list the {selector} selector: {synopsis}"
        );
    }
    Ok(())
}

/// Verify the fake generator renders a missing marker for a set identifier.
///
/// This is the harness's own control: it proves the artefact assertion above
/// can fail, so reintroducing the derived `netsuke-usage` identifier is caught
/// by the generated page and not only by the metadata assertion.
///
/// # Errors
///
/// Returns an error when the metadata, script run, or artefact assertions
/// cannot be satisfied.
#[test]
fn fake_generator_renders_a_missing_marker_for_a_set_synopsis_id() -> Result<()> {
    let temp = tempfile::tempdir().context("create release-help control tempdir")?;
    let bin_dir = temp.path().join("bin");
    install_fake_generator(&bin_dir)?;

    let mut metadata = serde_json::to_value(ReleaseHelpCli::get_doc_metadata())
        .context("serialise release-help metadata to JSON")?;
    let object = metadata
        .as_object_mut()
        .context("release-help metadata should serialise as a JSON object")?;
    object.insert(
        "synopsis_id".to_owned(),
        Value::String("netsuke-usage".to_owned()),
    );
    let metadata_path = temp.path().join("release-help-metadata.json");
    write_metadata(&metadata_path, &metadata)?;

    let out_dir = temp.path().join("out");
    let output = run_release_help(&bin_dir, &metadata_path, &out_dir)?;
    ensure!(
        output.status.success(),
        "release-help script failed: {output:?}"
    );

    let man_page = fs::read_to_string(out_dir.join("man/man1/netsuke.1"))
        .context("read the generated manual page")?;
    ensure!(
        man_page.contains("[missing: netsuke-usage]"),
        "the fake generator should model cargo-orthohelp's missing-identifier \
         rendering: {man_page}"
    );
    Ok(())
}
