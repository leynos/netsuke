//! End-to-end tests for `make verus` against the pinned real verifier.
//!
//! The other prover-tool suites replace the verifier with a fake so they can
//! run anywhere. These cases do the opposite: they drive the real Make target,
//! the pinned `rust-prover-tools` CLI, and the pinned Verus binary, so the
//! default proof, an ordered batch, and failure propagation are exercised
//! through the whole stack.
//!
//! Verus is an optional maintainer tool and the repository deliberately has no
//! Verus CI job, so a host without the pinned install skips these cases. The
//! `test-verus-e2e` target sets `NETSUKE_VERUS_E2E_STRICT=1`, which turns that
//! skip into a failure for a maintainer who means to run them.

#![cfg(unix)]

use anyhow::{Context, Result, ensure};
use proptest::prelude::*;
use std::fmt::Write as _;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::{TempDir, tempdir};

/// Environment variable that turns a missing verifier into a failure.
const STRICT_VARIABLE: &str = "NETSUKE_VERUS_E2E_STRICT";

/// Return the pinned Verus version recorded in `tools/verus/VERSION`.
///
/// # Errors
///
/// Returns an error if the pin cannot be read.
fn pinned_version() -> Result<String> {
    let contents = fs::read_to_string("tools/verus/VERSION").context("read tools/verus/VERSION")?;
    Ok(contents.trim().to_owned())
}

/// Return the install directory the pinned CLI uses for the pinned version.
fn verifier_root(version: &str) -> PathBuf {
    Path::new(".verus").join(version).join("verus")
}

/// Report whether the pinned real verifier is installed.
///
/// # Errors
///
/// Returns an error if the pin cannot be read, or if strict mode is set and the
/// verifier is absent.
#[expect(
    clippy::disallowed_methods,
    reason = "this is the single site that reads the strict-mode switch; the \
              test binary has no injected environment to consult"
)]
fn require_real_verifier() -> Result<bool> {
    let version = pinned_version()?;
    let installed = verifier_root(&version).is_dir();
    if installed {
        return Ok(true);
    }
    let strict = std::env::var_os(STRICT_VARIABLE).is_some_and(|value| value == "1");
    ensure!(
        !strict,
        "the pinned Verus {version} is not installed at {}; run `make install-verus`",
        verifier_root(&version).display()
    );
    Ok(false)
}

/// Run `make verus` with the supplied `VERUS_FLAGS` value.
///
/// # Errors
///
/// Returns an error if the Make invocation cannot be spawned.
fn run_make_verus(verus_flags: Option<&str>) -> Result<Output> {
    let mut command = Command::new("make");
    command
        .arg("--no-print-directory")
        .arg("-f")
        .arg("Makefile");
    if let Some(flags) = verus_flags {
        command.arg(format!("VERUS_FLAGS={flags}"));
    }
    command.arg("verus").output().context("run make verus")
}

/// Run `make verus` with `VERUS_FLAGS` and extra environment for the verifier.
///
/// The environment reaches the fake verifier through `make` and the pinned
/// CLI, so it is confined to the child process tree.
///
/// # Errors
///
/// Returns an error if the Make invocation cannot be spawned.
fn run_make_verus_with_env(verus_flags: &str, environment: &[(&str, String)]) -> Result<Output> {
    let mut command = Command::new("make");
    command
        .arg("--no-print-directory")
        .arg("-f")
        .arg("Makefile")
        .arg(format!("VERUS_FLAGS={verus_flags}"));
    for (key, value) in environment {
        command.env(key, value);
    }
    command
        .arg("verus")
        .output()
        .context("run make verus with a fake verifier")
}

/// Write a fake verifier that records each proof file it is asked to verify.
///
/// The pinned CLI resolves the binary through `--verus-bin`, so this exercises
/// its real batching loop without a Verus installation. `FAKE_VERUS_FAIL` names
/// the proof that exits with `FAKE_VERUS_STATUS`; every other proof succeeds.
///
/// # Errors
///
/// Returns an error if the script cannot be written or marked executable.
fn write_fake_verus(dir: &Path) -> Result<PathBuf> {
    let executable = dir.join("fake-verus");
    let script = concat!(
        "#!/bin/sh\n",
        "printf '%s\\n' \"$1\" >> \"$FAKE_VERUS_LOG\"\n",
        "if [ \"$1\" = \"--version\" ]; then\n",
        "    printf 'Verus\\n  Version: %s\\n  Profile: release\\n",
        "  Platform: linux_x86_64\\n",
        "  Toolchain: 1.98.1-x86_64-unknown-linux-gnu\\n' \"$FAKE_VERUS_PIN\"\n",
        "    exit 0\n",
        "fi\n",
        "if [ -n \"$FAKE_VERUS_FAIL\" ] && [ \"$1\" = \"$FAKE_VERUS_FAIL\" ]; then\n",
        "    exit \"$FAKE_VERUS_STATUS\"\n",
        "fi\n",
        "printf 'verification results:: 1 verified, 0 errors\\n'\n",
    );
    fs::write(&executable, script)
        .with_context(|| format!("write fake verifier to {}", executable.display()))?;
    let mut permissions = fs::metadata(&executable)
        .with_context(|| format!("stat fake verifier {}", executable.display()))?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable, permissions)
        .with_context(|| format!("mark {} executable", executable.display()))?;
    Ok(executable)
}

/// A batch of real proof files plus the fake verifier that runs them.
struct BatchFixture {
    _temp: TempDir,
    verifier: PathBuf,
    proofs: Vec<PathBuf>,
    log: PathBuf,
    pinned: String,
}

impl BatchFixture {
    /// Stage `count` proof files and a fake verifier in a temporary directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the pin cannot be read or a fixture file cannot be
    /// written.
    fn new(count: usize) -> Result<Self> {
        let temp = tempdir().context("create batch fixture directory")?;
        let mut proofs = Vec::with_capacity(count);
        for index in 0..count {
            let path = temp.path().join(format!("proof-{index}.rs"));
            fs::write(&path, "// proof\n")
                .with_context(|| format!("write proof file {}", path.display()))?;
            proofs.push(path);
        }
        let verifier = write_fake_verus(temp.path())?;
        let log = temp.path().join("invocations.log");
        Ok(Self {
            _temp: temp,
            verifier,
            proofs,
            log,
            pinned: pinned_version()?,
        })
    }

    /// Run the batch, failing the proof at `failure` when one is supplied.
    ///
    /// # Errors
    ///
    /// Returns an error if the Make invocation cannot be spawned.
    fn run(&self, failure: Option<(usize, i32)>) -> Result<Output> {
        let mut flags = format!(
            "--verus-bin {} --no-ensure-toolchain --no-install-missing",
            self.verifier.display()
        );
        for proof in &self.proofs {
            write!(flags, " --proof-file {}", proof.display())
                .context("append a proof file to the verifier flags")?;
        }
        let failing = failure
            .and_then(|(index, _)| self.proofs.get(index))
            .map_or_else(String::new, |proof| proof.display().to_string());
        let status = failure.map_or_else(|| "0".to_owned(), |(_, code)| code.to_string());
        let environment = [
            ("FAKE_VERUS_LOG", self.log.display().to_string()),
            ("FAKE_VERUS_PIN", self.pinned.clone()),
            ("FAKE_VERUS_FAIL", failing),
            ("FAKE_VERUS_STATUS", status),
        ];
        run_make_verus_with_env(&flags, &environment)
    }

    /// Read the proof files the verifier was asked to verify, in order.
    ///
    /// # Errors
    ///
    /// Returns an error if the invocation log cannot be read.
    fn invocations(&self) -> Result<Vec<String>> {
        let contents = fs::read_to_string(&self.log)
            .with_context(|| format!("read {}", self.log.display()))?;
        Ok(contents.lines().map(str::to_owned).collect())
    }

    /// Return the proof paths as the verifier should have received them.
    fn proof_paths(&self) -> Vec<String> {
        self.proofs
            .iter()
            .map(|path| path.display().to_string())
            .collect()
    }
}

/// Generate a batch length, a failing proof index, and that proof's status.
fn failure_cases() -> impl Strategy<Value = (usize, usize, i32)> {
    (1usize..=4).prop_flat_map(|count| {
        (0..count, 1i32..=125).prop_map(move |(index, status)| (count, index, status))
    })
}

/// Count the verifier's per-proof result lines in a run's output.
fn verification_runs(output: &Output) -> usize {
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    combined.matches("verification results::").count()
}

/// Check that the default target verifies the repository's proof kernel.
#[test]
fn verus_verifies_the_default_proof_with_the_real_verifier() -> Result<()> {
    if !require_real_verifier()? {
        return Ok(());
    }

    let output = run_make_verus(None)?;

    ensure!(
        output.status.success(),
        "make verus should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    ensure!(
        verification_runs(&output) == 1,
        "the default target should run exactly one proof"
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    ensure!(
        combined.contains("0 errors"),
        "the real verifier should report no errors, got `{combined}`"
    );
    Ok(())
}

/// Check that repeated proof-file options run in order against the real
/// verifier.
#[test]
fn verus_runs_an_ordered_batch_with_the_real_verifier() -> Result<()> {
    if !require_real_verifier()? {
        return Ok(());
    }

    let output = run_make_verus(Some(
        "--proof-file verus/canonical_value/main.rs --proof-file verus/canonical_value/main.rs",
    ))?;

    ensure!(
        output.status.success(),
        "make verus should succeed for a repeated proof: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    ensure!(
        verification_runs(&output) == 2,
        "the batch should run the proof twice, got {} runs",
        verification_runs(&output)
    );
    Ok(())
}

/// Check that a real verification failure reaches the caller's exit status.
#[test]
fn verus_propagates_a_real_verification_failure() -> Result<()> {
    if !require_real_verifier()? {
        return Ok(());
    }

    let temp = tempdir().context("create failing proof directory")?;
    let failing_proof = temp.path().join("failing.rs");
    fs::write(
        &failing_proof,
        concat!(
            "use vstd::prelude::*;\n",
            "\n",
            "verus! {\n",
            "\n",
            "proof fn deliberate_failure()\n",
            "    ensures 1 + 1 == 3,\n",
            "{\n",
            "}\n",
            "\n",
            "/// Provide the entry point Verus requires; it carries no proof of its own.\n",
            "fn main() {}\n",
            "\n",
            "} // verus!\n",
        ),
    )
    .context("write failing proof")?;

    let output = run_make_verus(Some(&format!("--proof-file {}", failing_proof.display())))?;

    ensure!(!output.status.success(), "make verus should fail");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    ensure!(
        combined.contains("postcondition not satisfied"),
        "the real verifier should report the failed postcondition, got `{combined}`"
    );
    ensure!(
        combined.contains("prover-tools: target=verus failed exit="),
        "the Make target should report the propagated failure, got `{combined}`"
    );
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(6))]

    /// Check that a successful batch runs every proof exactly once, in order.
    ///
    /// The pinned CLI is the real one; only the verifier is fake, so the
    /// ordering asserted here is the CLI's own batching behaviour.
    #[test]
    fn verus_runs_every_proof_of_a_successful_batch_in_order(count in 1usize..=4) {
        if !require_real_verifier().expect("check the pinned verifier") {
            return Ok(());
        }
        let fixture = BatchFixture::new(count).expect("stage the batch fixture");

        let output = fixture.run(None).expect("run the batch");

        prop_assert!(
            output.status.success(),
            "a successful batch should exit zero: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let mut expected = vec!["--version".to_owned()];
        expected.extend(fixture.proof_paths());
        prop_assert_eq!(fixture.invocations().expect("read invocations"), expected);
    }

    /// Check that a failing batch stops at the failing proof and reports its
    /// status.
    ///
    /// The executed prefix is the contract: proofs before the failure ran, the
    /// failing proof ran, and nothing after it did.
    #[test]
    fn verus_stops_at_the_first_failing_proof_and_propagates_its_status(
        (count, index, status) in failure_cases()
    ) {
        if !require_real_verifier().expect("check the pinned verifier") {
            return Ok(());
        }
        let fixture = BatchFixture::new(count).expect("stage the batch fixture");

        let output = fixture.run(Some((index, status))).expect("run the batch");

        prop_assert!(!output.status.success(), "a failing batch should not exit zero");
        let mut expected = vec!["--version".to_owned()];
        expected.extend(fixture.proof_paths().into_iter().take(index + 1));
        prop_assert_eq!(fixture.invocations().expect("read invocations"), expected);
        let stderr = String::from_utf8_lossy(&output.stderr);
        prop_assert!(
            stderr.contains(&format!("prover-tools: target=verus failed exit={status}")),
            "the target should report exit {status}, got `{stderr}`"
        );
    }
}
