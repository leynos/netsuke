//! Contract model for Cargo's Rustdoc environment in Makefile recipes.
//!
//! The caller-overridable `RUSTDOC_FLAGS` Make variable carries the shared
//! default Rustdoc configuration, but Cargo accepts only `RUSTDOCFLAGS`.
//! Make exports the supported name directly, which preserves literal caller
//! values without re-parsing them in a shell. It explicitly unexports the
//! unsupported name, preventing Cargo from warning about it. The Unix
//! behavioural test exercises that real Make-to-Cargo boundary with a fake
//! Cargo executable; static checks keep the safe export shape intact.

use super::{read_repo_file, target_recipe};
use anyhow::{Context, Result, ensure};
use camino::Utf8Path;

#[cfg(unix)]
use assert_cmd::Command;
#[cfg(unix)]
use std::path::Path;
#[cfg(unix)]
use tempfile::tempdir;
#[cfg(unix)]
use test_support::{fs, write_exec_with_content};

/// The caller-overridable Make default for the Rustdoc configuration.
const RUSTDOC_FLAGS_DEFAULT: &str = "RUSTDOC_FLAGS ?= --cfg docsrs -D warnings";
/// The directive that prevents Cargo from receiving its unsupported name.
const RUSTDOC_FLAGS_UNEXPORT: &str = "unexport RUSTDOC_FLAGS";
/// The safe Make-level mapping to Cargo's supported environment variable.
const RUSTDOCFLAGS_EXPORT: &str = "export RUSTDOCFLAGS := $(RUSTDOC_FLAGS)";

#[cfg(unix)]
/// An override whose quotes must reach Rustdoc unchanged.
const QUOTED_RUSTDOC_FLAGS: &str = "--cfg feature=\"x\"";

#[test]
fn behavioural_rustdocflags_default_preserves_rustdoc_warning_denial() -> Result<()> {
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;
    ensure!(
        makefile.lines().any(|line| line == RUSTDOC_FLAGS_DEFAULT),
        "Makefile should retain {RUSTDOC_FLAGS_DEFAULT:?}"
    );
    Ok(())
}

#[test]
fn behavioural_make_exports_supported_rustdocflags_after_unexporting_unsupported_name() -> Result<()>
{
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;
    let default_index = makefile
        .lines()
        .position(|line| line == RUSTDOC_FLAGS_DEFAULT)
        .context("Makefile should declare the RUSTDOC_FLAGS default")?;
    let unexport_index = makefile
        .lines()
        .position(|line| line == RUSTDOC_FLAGS_UNEXPORT)
        .context("Makefile should unexport the unsupported RUSTDOC_FLAGS name")?;
    let export_index = makefile
        .lines()
        .position(|line| line == RUSTDOCFLAGS_EXPORT)
        .context("Makefile should export Cargo's supported RUSTDOCFLAGS name")?;

    ensure!(
        default_index < unexport_index && unexport_index < export_index,
        "the RUSTDOC_FLAGS default, unexport, and supported export must remain ordered"
    );
    Ok(())
}

#[test]
fn behavioural_rustdoc_consumers_rely_on_the_safe_make_export() -> Result<()> {
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;
    for target in ["doctest", "lint-clippy", "doc-coverage"] {
        let recipe = target_recipe(&makefile, target)
            .with_context(|| format!("Makefile should declare a {target} target"))?;
        ensure!(
            !recipe.contains("RUSTDOCFLAGS="),
            "{target} must rely on Make's exported RUSTDOCFLAGS rather than interpolate flags in a shell: {recipe:?}"
        );
    }
    Ok(())
}

#[test]
fn behavioural_makefile_never_exports_or_shell_expands_rustdoc_flags() -> Result<()> {
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;
    for (line_number, line) in makefile.lines().enumerate() {
        let is_recipe = line.starts_with('\t');
        ensure!(
            !line.trim_start().starts_with("export RUSTDOC_FLAGS"),
            "Makefile line {} must not export unsupported RUSTDOC_FLAGS: {line:?}",
            line_number + 1
        );
        ensure!(
            !(is_recipe && line.contains("$${RUSTDOC_FLAGS}")),
            "Makefile recipe line {} must not shell-expand RUSTDOC_FLAGS: {line:?}",
            line_number + 1
        );
        ensure!(
            !(is_recipe && line.contains("RUSTDOC_FLAGS=") && line.contains("$(CARGO)")),
            "Makefile recipe line {} must not pass unsupported RUSTDOC_FLAGS to Cargo: {line:?}",
            line_number + 1
        );
    }
    Ok(())
}

#[cfg(unix)]
/// Run `doctest` with a caller-supplied Rustdoc override through one source.
fn run_doctest_with_override(
    fake_cargo: &Path,
    log: &Path,
    source: &str,
    command_line: bool,
) -> Result<String> {
    let mut make = Command::new("make");
    make.current_dir(env!("CARGO_MANIFEST_DIR"))
        .arg("--no-print-directory")
        .arg("-f")
        .arg("Makefile")
        .env("CARGO", fake_cargo)
        .env("RUSTDOC_ENVIRONMENT_LOG", log);
    if command_line {
        make.arg(format!("RUSTDOC_FLAGS={QUOTED_RUSTDOC_FLAGS}"));
    } else {
        make.env("RUSTDOC_FLAGS", QUOTED_RUSTDOC_FLAGS);
    }
    make.arg("doctest");

    let output = make
        .output()
        .with_context(|| format!("run doctest with a {source} Rustdoc override"))?;
    ensure!(
        output.status.success(),
        "doctest with a {source} override should call fake Cargo successfully: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::read_to_string(log).context("read fake Cargo environment log")
}

#[cfg(unix)]
/// Assert that Cargo receives the exact supported Rustdoc configuration.
fn assert_doctest_record(record: &str, source: &str) -> Result<()> {
    let mut fields = record.lines().map(|line| {
        line.split_once('\t')
            .with_context(|| format!("malformed fake Cargo record {line:?}"))
    });
    let rustdocflags = fields
        .next()
        .transpose()?
        .context("fake Cargo record should include RUSTDOCFLAGS")?;
    let unsupported = fields
        .next()
        .transpose()?
        .context("fake Cargo record should include RUSTDOC_FLAGS presence")?;
    let arguments = fields
        .next()
        .transpose()?
        .context("fake Cargo record should include arguments")?;

    ensure!(
        rustdocflags == ("rustdocflags", QUOTED_RUSTDOC_FLAGS),
        "a {source} override must retain its literal quoted Rustdoc value, found {rustdocflags:?}"
    );
    ensure!(
        unsupported == ("unsupported", ""),
        "a {source} override must not leak RUSTDOC_FLAGS to Cargo, found {unsupported:?}"
    );
    ensure!(
        arguments.0 == "arguments"
            && arguments
                .1
                .contains("test --workspace --doc --all-features"),
        "doctest should invoke Cargo's documentation-test arguments, found {arguments:?}"
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn behavioural_doctest_passes_quote_bearing_overrides_only_as_rustdocflags() -> Result<()> {
    let temporary = tempdir().context("create fake Cargo directory")?;
    let log = temporary.path().join("cargo-environment.log");
    let fake_cargo = write_exec_with_content(
        temporary.path(),
        "cargo",
        concat!(
            "#!/bin/sh\n",
            ": \"${RUSTDOC_ENVIRONMENT_LOG:?}\"\n",
            "{\n",
            "  printf 'rustdocflags\\t%s\\n' \"${RUSTDOCFLAGS-}\"\n",
            "  printf 'unsupported\\t%s\\n' \"${RUSTDOC_FLAGS+present}\"\n",
            "  printf 'arguments\\t%s\\n' \"$*\"\n",
            "} > \"$RUSTDOC_ENVIRONMENT_LOG\"\n"
        ),
    )
    .context("write fake Cargo executable")?;

    for (source, command_line) in [("environment", false), ("command line", true)] {
        let record = run_doctest_with_override(&fake_cargo, &log, source, command_line)?;
        assert_doctest_record(&record, source)?;
    }
    Ok(())
}
