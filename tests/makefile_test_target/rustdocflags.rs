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
use proptest::prelude::*;
#[cfg(unix)]
use std::path::{Path, PathBuf};
#[cfg(unix)]
use tempfile::tempdir;
#[cfg(unix)]
use test_support::{fs, write_exec_with_content};

/// The caller-overridable Make default for the Rustdoc configuration.
const RUSTDOC_FLAGS_DEFAULT: &str = "RUSTDOC_FLAGS ?= --cfg docsrs -D warnings";
/// The directive that prevents Cargo from receiving its unsupported name.
const RUSTDOC_FLAGS_UNEXPORT: &str = "unexport RUSTDOC_FLAGS";
/// The safe Make-level mapping to Cargo's supported environment variable.
const RUSTDOCFLAGS_EXPORT: &str = "export RUSTDOCFLAGS := $(value RUSTDOC_FLAGS)";

/// An override whose shell-sensitive values must reach Rustdoc unchanged.
#[cfg(unix)]
const SHELL_SENSITIVE_RUSTDOC_FLAGS: &str = r#"--cfg marker="price$5 \"path\\name\"; &"#;

/// Describe one caller-supplied Rustdoc override source and value.
#[cfg(unix)]
struct RustdocOverride<'flags> {
    /// Store the exact Rustdoc flags supplied by the caller.
    rustdoc_flags: &'flags str,
    /// Identify where Make receives the caller override.
    source: &'static str,
    /// Distinguish a Make command-line assignment from an environment value.
    command_line: bool,
}

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

/// Run `doctest` with a caller-supplied Rustdoc override through one source.
#[cfg(unix)]
fn run_doctest_with_override(
    fake_cargo: &Path,
    log: &Path,
    override_config: &RustdocOverride<'_>,
) -> Result<String> {
    let mut make = Command::new("make");
    make.current_dir(env!("CARGO_MANIFEST_DIR"))
        .arg("--no-print-directory")
        .arg("-f")
        .arg("Makefile")
        .env("CARGO", fake_cargo)
        .env("RUSTDOC_ENVIRONMENT_LOG", log);
    if override_config.command_line {
        make.arg(format!("RUSTDOC_FLAGS={}", override_config.rustdoc_flags));
    } else {
        make.env("RUSTDOC_FLAGS", override_config.rustdoc_flags);
    }
    make.arg("doctest");

    let output = make.output().with_context(|| {
        format!(
            "run doctest with a {} Rustdoc override",
            override_config.source
        )
    })?;
    ensure!(
        output.status.success(),
        "doctest with a {} override should call fake Cargo successfully: {}",
        override_config.source,
        String::from_utf8_lossy(&output.stderr)
    );
    fs::read_to_string(log).context("read fake Cargo environment log")
}

/// Assert that Cargo receives the exact supported Rustdoc configuration.
#[cfg(unix)]
fn assert_doctest_record(record: &str, rustdoc_flags: &str, source: &str) -> Result<()> {
    let mut fields = record.lines().map(|line| {
        line.split_once('\t')
            .with_context(|| format!("malformed fake Cargo record {line:?}"))
    });
    let cargo_rustdocflags = fields
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
        cargo_rustdocflags == ("rustdocflags", rustdoc_flags),
        "a {source} override must retain its literal Rustdoc value, found {cargo_rustdocflags:?}"
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

/// Create a Cargo stand-in that records its environment and arguments.
#[cfg(unix)]
fn write_fake_cargo(directory: &Path) -> Result<PathBuf> {
    write_exec_with_content(
        directory,
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
    .context("write fake Cargo executable")
}

/// Generate valid, shell-sensitive Rustdoc overrides with bounded entropy.
#[cfg(unix)]
fn rustdoc_flags_strategy() -> impl Strategy<Value = String> {
    "[a-z]{0,12}".prop_map(|suffix| format!(r#"--cfg marker="price$5 \"path\\name\"; & {suffix}""#))
}

#[cfg(unix)]
#[test]
fn behavioural_doctest_passes_shell_sensitive_overrides_only_as_rustdocflags() -> Result<()> {
    let temporary = tempdir().context("create fake Cargo directory")?;
    let log = temporary.path().join("cargo-environment.log");
    let fake_cargo = write_fake_cargo(temporary.path())?;

    for (source, command_line) in [("environment", false), ("command line", true)] {
        let override_config = RustdocOverride {
            rustdoc_flags: SHELL_SENSITIVE_RUSTDOC_FLAGS,
            source,
            command_line,
        };
        let record = run_doctest_with_override(&fake_cargo, &log, &override_config)?;
        assert_doctest_record(&record, SHELL_SENSITIVE_RUSTDOC_FLAGS, source)?;
    }
    Ok(())
}

#[cfg(unix)]
proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    #[test]
    fn property_doctest_preserves_rustdoc_override_values(
        rustdoc_flags in rustdoc_flags_strategy(),
    ) {
        let temporary = tempdir().expect("create fake Cargo directory");
        let log = temporary.path().join("cargo-environment.log");
        let fake_cargo = write_fake_cargo(temporary.path()).expect("write fake Cargo executable");

        for (source, command_line) in [("environment", false), ("command line", true)] {
            let override_config = RustdocOverride {
                rustdoc_flags: &rustdoc_flags,
                source,
                command_line,
            };
            let record = run_doctest_with_override(
                &fake_cargo,
                &log,
                &override_config,
            )
            .expect("run doctest through fake Cargo");
            assert_doctest_record(&record, &rustdoc_flags, source)
                .expect("preserve the Rustdoc override in Cargo's environment");
        }
    }
}
