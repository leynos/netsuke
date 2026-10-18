//! Contract model for Cargo's Rustdoc environment in Makefile recipes.
//!
//! The caller-overridable `RUSTDOC_FLAGS` Make variable carries the shared
//! default Rustdoc configuration, but Cargo accepts only `RUSTDOCFLAGS`.
//! Make exports the supported name directly, which preserves literal caller
//! values without re-parsing them in a shell. It explicitly unexports the
//! unsupported name, preventing Cargo from warning about it. The behavioural
//! test exercises that real Make-to-Cargo boundary with a fake Cargo
//! executable; static checks keep the safe export shape intact.
//!
//! Those behavioural tests inject the build-tools boundary. `doctest` depends
//! on `check-build-tools`, so a test that ran Make against the ambient host
//! would cross the real capability preflight and fail on any machine without
//! the pinned `mold` — reporting a linker fault when its subject is
//! `RUSTDOCFLAGS` propagation, and telling a reader nothing about RUSTDOCFLAGS
//! when it did so. The sandbox supplies a fake `mold` at the committed pin and
//! a fake `rustup`, so these tests reach the fake Cargo on a host with neither
//! tool. Real-preflight validation belongs to `build_tools_check_tests.rs` and
//! `build_tools_pin_tests.rs`; the `doctest` case added to the former is what
//! holds the prerequisite relation this module deliberately stops depending on.
//!
//! The harness-backed items are Linux-only, matching every other `Sandbox`
//! consumer in the suite. `Sandbox::new` resolves its whole utility allowlist
//! eagerly and that list names `sha256sum`, which GNU coreutils provides and
//! stock macOS does not, so a wider gate would fail while building the
//! sandbox rather than while checking `RUSTDOCFLAGS`. The four static contract
//! tests read only the committed Makefile and stay platform-neutral, so macOS
//! still checks the export shape; only the behavioural Make-to-Cargo
//! observation is confined to Linux.

use super::{read_repo_file, target_recipe};
use anyhow::{Context, Result, ensure};
use camino::Utf8Path;

#[cfg(all(unix, target_os = "linux"))]
use camino::Utf8PathBuf;
#[cfg(all(unix, target_os = "linux"))]
use proptest::prelude::*;
#[cfg(all(unix, target_os = "linux"))]
use rstest::rstest;
#[cfg(all(unix, target_os = "linux"))]
use std::io::ErrorKind;
#[cfg(all(unix, target_os = "linux"))]
use std::path::Path;
#[cfg(all(unix, target_os = "linux"))]
use test_support::build_tools::{
    MakeInvocation, Sandbox, combined, pinned_mold_version, pinned_toolchain,
};
#[cfg(all(unix, target_os = "linux"))]
use test_support::{fs, write_exec_with_content};

/// The caller-overridable Make default for the Rustdoc configuration.
const RUSTDOC_FLAGS_DEFAULT: &str = "RUSTDOC_FLAGS ?= --cfg docsrs -D warnings";
/// The directive that prevents Cargo from receiving its unsupported name.
const RUSTDOC_FLAGS_UNEXPORT: &str = "unexport RUSTDOC_FLAGS";
/// The safe Make-level mapping to Cargo's supported environment variable.
const RUSTDOCFLAGS_EXPORT: &str = "export RUSTDOCFLAGS := $(value RUSTDOC_FLAGS)";

/// An override whose shell-sensitive values must reach Rustdoc unchanged.
#[cfg(all(unix, target_os = "linux"))]
const SHELL_SENSITIVE_RUSTDOC_FLAGS: &str = r#"--cfg marker="price$5 \"path\\name\"; &"#;

/// The variable naming the file the fake Cargo records into.
#[cfg(all(unix, target_os = "linux"))]
const ENVIRONMENT_LOG: &str = "RUSTDOC_ENVIRONMENT_LOG";

/// Describe one caller-supplied Rustdoc override source and value.
#[cfg(all(unix, target_os = "linux"))]
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

/// A `doctest` run whose build-tools prerequisite is satisfied by fakes.
///
/// The sandbox supplies the `mold` and `rustup` that `check-build-tools`
/// demands, so the recipe reaches the fake Cargo however the host is
/// provisioned. See the module comment for why that boundary is injected
/// rather than crossed.
#[cfg(all(unix, target_os = "linux"))]
struct DoctestHarness {
    /// The hermetic `PATH` and `HOME`, and the install prefix mold sits under.
    sandbox: Sandbox,
    /// The fake Cargo, which records what the recipe exported to it.
    cargo: Utf8PathBuf,
    /// Where that fake writes its record; rewritten on every invocation.
    log: Utf8PathBuf,
}

#[cfg(all(unix, target_os = "linux"))]
impl DoctestHarness {
    /// Build a sandbox holding the fake Cargo and both pinned tools.
    fn new() -> Result<Self> {
        let sandbox = Sandbox::new()?;
        // Mold goes under the install prefix, which the Makefile puts first on
        // `PATH` and which `check-build-tools` reports the resolved path of.
        sandbox.write_mold(&sandbox.prefix().join("bin"), &pinned_mold_version()?)?;
        sandbox.write_rustup(&pinned_toolchain()?)?;
        let cargo = write_fake_cargo(sandbox.bin().as_std_path())?;
        let log = sandbox.home().join(ENVIRONMENT_LOG);
        Ok(Self {
            sandbox,
            cargo,
            log,
        })
    }

    /// Run `doctest` with one caller-supplied Rustdoc override.
    ///
    /// Returns the fake Cargo's record of what the recipe handed it. The log
    /// is removed first, so a record read here is always this run's: the fake
    /// truncates what it writes, and removing the file additionally means a
    /// run that never reached Cargo fails rather than reporting the previous
    /// case's values.
    fn run(&self, override_config: &RustdocOverride<'_>) -> Result<String> {
        discard_log(&self.log)?;
        let mut invocation = MakeInvocation::new("doctest")
            .variable("CARGO", &self.cargo)
            .environment(ENVIRONMENT_LOG, &self.log);
        // Both routes must stay: the distinction between a Make command-line
        // assignment and an environment value is what this test exists to
        // hold.
        invocation = if override_config.command_line {
            invocation.variable("RUSTDOC_FLAGS", override_config.rustdoc_flags)
        } else {
            invocation.environment("RUSTDOC_FLAGS", override_config.rustdoc_flags)
        };

        let output = self.sandbox.run_make(&invocation).with_context(|| {
            format!(
                "run doctest with a {} Rustdoc override",
                override_config.source
            )
        })?;
        // The whole of both streams, not just stderr: a Make failure reports
        // its cause on whichever stream the recipe used, and a reader of this
        // message has only the message.
        ensure!(
            output.status.success(),
            "doctest with a {} override should call fake Cargo successfully: {}",
            override_config.source,
            combined(&output)
        );
        fs::read_to_string(self.log.as_std_path()).context("read fake Cargo environment log")
    }
}

/// Remove a previous run's environment log, tolerating its absence.
#[cfg(all(unix, target_os = "linux"))]
fn discard_log(log: &Utf8Path) -> Result<()> {
    match fs::remove_file(log.as_std_path()) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("remove stale {log}")),
    }
}

/// Assert that Cargo receives the exact supported Rustdoc configuration.
#[cfg(all(unix, target_os = "linux"))]
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
///
/// The recipe reaches this through the `CARGO` Make variable, so it needs no
/// name on `PATH`; it is placed in the sandbox `bin` because that is a
/// directory the harness already owns.
#[cfg(all(unix, target_os = "linux"))]
fn write_fake_cargo(directory: &Path) -> Result<Utf8PathBuf> {
    let stub = write_exec_with_content(
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
    .context("write fake Cargo executable")?;
    Utf8PathBuf::from_path_buf(stub)
        .map_err(|non_utf8| anyhow::anyhow!("fake Cargo path {} is not UTF-8", non_utf8.display()))
}

/// Generate valid, shell-sensitive Rustdoc overrides with bounded entropy.
#[cfg(all(unix, target_os = "linux"))]
fn rustdoc_flags_strategy() -> impl Strategy<Value = String> {
    "[a-z]{0,12}".prop_map(|suffix| format!(r#"--cfg marker="price$5 \"path\\name\"; & {suffix}""#))
}

#[cfg(all(unix, target_os = "linux"))]
#[rstest]
#[case("environment", false)]
#[case("command line", true)]
fn behavioural_doctest_passes_shell_sensitive_overrides_only_as_rustdocflags(
    #[case] source: &'static str,
    #[case] command_line: bool,
) -> Result<()> {
    // One sandbox per case rather than per test: each case is an independent
    // claim about one override route, and a shared log would let a case that
    // never reached Cargo read the other case's record.
    let harness = DoctestHarness::new()?;
    let override_config = RustdocOverride {
        rustdoc_flags: SHELL_SENSITIVE_RUSTDOC_FLAGS,
        source,
        command_line,
    };
    let record = harness.run(&override_config)?;
    assert_doctest_record(&record, SHELL_SENSITIVE_RUSTDOC_FLAGS, source)
}

#[cfg(all(unix, target_os = "linux"))]
proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    #[test]
    fn property_doctest_preserves_rustdoc_override_values(
        rustdoc_flags in rustdoc_flags_strategy(),
    ) {
        // A harness per case: building the sandbox is the expensive part of
        // this test, and 16 cases × 2 sources would otherwise stage 32.
        let harness = DoctestHarness::new().expect("build the doctest harness");

        for (source, command_line) in [("environment", false), ("command line", true)] {
            let override_config = RustdocOverride {
                rustdoc_flags: &rustdoc_flags,
                source,
                command_line,
            };
            let record = harness
                .run(&override_config)
                .expect("run doctest through fake Cargo");
            assert_doctest_record(&record, &rustdoc_flags, source)
                .expect("preserve the Rustdoc override in Cargo's environment");
        }
    }
}
