//! Fixtures and helpers for the `make verus` end-to-end tests.
//!
//! The cases in the parent module need three things: the pinned Verus version
//! and its install location, a way to run `make verus` with a chosen
//! `VERUS_FLAGS` value, and a fake verifier that records the proof files the
//! pinned CLI asks it to verify. The fake verifier is what lets the batching
//! loop be exercised without a Verus installation; the real-verifier cases use
//! the same runner with no fake in place.

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
pub fn pinned_version() -> Result<String> {
    let contents = fs::read_to_string("tools/verus/VERSION").context("read tools/verus/VERSION")?;
    Ok(contents.trim().to_owned())
}

/// Return the install directory the pinned CLI uses for the pinned version.
pub fn verifier_root(version: &str) -> PathBuf {
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
pub fn require_real_verifier() -> Result<bool> {
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

/// Build a `make verus` command before its `VERUS_FLAGS` value is chosen.
///
/// The recipe reads `VERUS_FLAGS` and `MAKEFLAGS` from the environment, so an
/// inherited value would silently replace the one a case supplies, or add Make
/// options the case never named. Removing them in one place keeps the two
/// runners from drifting apart on that rule.
fn make_verus_command() -> Command {
    let mut command = Command::new("make");
    command
        .env_remove("VERUS_FLAGS")
        .env_remove("MAKEFLAGS")
        .arg("--no-print-directory")
        .arg("-f")
        .arg("Makefile");
    command
}

/// Run `make verus` with the supplied `VERUS_FLAGS` value.
///
/// # Errors
///
/// Returns an error if the Make invocation cannot be spawned.
pub fn run_make_verus(verus_flags: Option<&str>) -> Result<Output> {
    let mut command = make_verus_command();
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
pub fn run_make_verus_with_env(
    verus_flags: &str,
    environment: &[(&str, String)],
) -> Result<Output> {
    let mut command = make_verus_command();
    command.arg(format!("VERUS_FLAGS={verus_flags}"));
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
pub fn write_fake_verus(dir: &Path) -> Result<PathBuf> {
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
pub struct BatchFixture {
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
    pub fn new(count: usize) -> Result<Self> {
        Self::in_order(&(0..count).collect::<Vec<_>>())
    }

    /// Stage proofs in an explicit order and return the fixture.
    ///
    /// `order` must be a permutation of `0..order.len()`. Each proof keeps the
    /// name of its original index, so a run that reorders the inputs, drops one
    /// or repeats one is visible in the invocation log rather than masked by
    /// the staged names.
    ///
    /// # Errors
    ///
    /// Returns an error if the pin cannot be read or a fixture file cannot be
    /// written.
    pub fn in_order(order: &[usize]) -> Result<Self> {
        let temp = tempdir().context("create batch fixture directory")?;
        let mut proofs = Vec::with_capacity(order.len());
        for index in order {
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
    pub fn run(&self, failure: Option<(usize, i32)>) -> Result<Output> {
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
    pub fn invocations(&self) -> Result<Vec<String>> {
        let contents = fs::read_to_string(&self.log)
            .with_context(|| format!("read {}", self.log.display()))?;
        Ok(contents.lines().map(str::to_owned).collect())
    }

    /// Return the proof paths as the verifier should have received them.
    pub fn proof_paths(&self) -> Vec<String> {
        self.proofs
            .iter()
            .map(|path| path.display().to_string())
            .collect()
    }
}

/// Generate a batch length, a failing proof index, and that proof's status.
///
/// The status is confined to `1..=125` because that is the range Make can
/// report unchanged. Above it the shell reserves 126 and 127 for "not
/// executable" and "not found", and 128 upwards denote termination by a signal,
/// so a proof that exited with one of those statuses would be reported as the
/// shell's meaning rather than its own. Generating them would assert against a
/// number the pipeline never propagates.
pub fn failure_cases() -> impl Strategy<Value = (usize, usize, i32)> {
    (1usize..=4).prop_flat_map(|count| {
        (0..count, 1i32..=125).prop_map(move |(index, status)| (count, index, status))
    })
}

/// Generate a batch order, the position within it that fails, and its status.
///
/// The order is a permutation rather than `0..count`, so a batch that silently
/// ran the proofs in creation order could not pass by accident. The statuses
/// are confined to `1..=125` for the reason [`failure_cases`] records.
pub fn ordered_failure_cases() -> impl Strategy<Value = (Vec<usize>, usize, i32)> {
    (1usize..=4).prop_flat_map(|count| {
        (permutation(count), 0..count, 1i32..=125)
            .prop_map(move |(order, index, status)| (order, index, status))
    })
}

/// Generate a random ordering of `0..count`.
///
/// Ordering the indices by a random key each is the simplest way to reach every
/// permutation without building one by hand; the index breaks ties so the
/// result is always a permutation, not merely a sort.
fn permutation(count: usize) -> impl Strategy<Value = Vec<usize>> {
    prop::collection::vec(any::<u64>(), count).prop_map(|keys| {
        let mut keyed: Vec<(u64, usize)> = keys.into_iter().zip(0..).collect();
        keyed.sort_unstable();
        keyed.into_iter().map(|(_, index)| index).collect()
    })
}

/// Combine a run's stdout and stderr in the order the verifier wrote them.
pub fn output_text(output: &Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    format!("{stdout}{stderr}")
}

/// Count the verifier's per-proof result lines in a run's output.
pub fn verification_runs(output: &Output) -> usize {
    let combined = output_text(output);
    combined.matches("verification results::").count()
}

/// Read the verified count from each per-proof result line, in order.
///
/// The real verifier prints one `verification results:: N verified` line per
/// proof and names no file, so the count is what distinguishes one proof's
/// result from another's.
pub fn verification_counts(output: &Output) -> Vec<usize> {
    let combined = output_text(output);
    combined
        .lines()
        .filter_map(|line| {
            let rest = line.split("verification results:: ").nth(1)?;
            rest.split(" verified").next()?.trim().parse().ok()
        })
        .collect()
}

/// Build a `VERUS_FLAGS` value naming two proof files in the given order.
pub fn batch_flags(first: &Path, second: &Path) -> String {
    format!(
        "--proof-file {} --proof-file {}",
        first.display(),
        second.display()
    )
}

/// Write a minimal proof whose verified count differs from the canonical
/// kernel's, so a batch's order is visible in the verifier's own output.
///
/// # Errors
///
/// Returns an error if the proof file cannot be written.
pub fn write_distinct_proof(dir: &Path) -> Result<PathBuf> {
    let path = dir.join("distinct.rs");
    fs::write(
        &path,
        concat!(
            "use vstd::prelude::*;\n",
            "\n",
            "verus! {\n",
            "\n",
            "/// A single trivial obligation, distinct from the canonical kernel.\n",
            "proof fn one_is_one()\n",
            "    ensures 1 + 1 == 2,\n",
            "{\n",
            "}\n",
            "\n",
            "fn main() {}\n",
            "\n",
            "} // verus!\n",
        ),
    )
    .with_context(|| format!("write distinct proof to {}", path.display()))?;
    Ok(path)
}
