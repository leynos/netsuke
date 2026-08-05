//! Starting points that more than one `dev-fast` test suite needs.
//!
//! Scope: these describe a *sandbox already in a known state*, not assertions
//! about it. [`BuildScenario`] is a sandbox where `dev-fast-check` passes;
//! [`InstallerScenario`] is one where a fake release is published and the
//! toolchain half is already satisfied, so a test can concentrate on the
//! linker half.
//!
//! Re-use policy: a scenario belongs here once a second suite needs it, and
//! only then. Suite-specific conveniences stay with their suite — the
//! installer tests keep their own `ChecksumFailure`, because a fixture that
//! encodes one suite's failure taxonomy is not shared ground. Keep these
//! constructors free of assertions so a scenario cannot decide, on a caller's
//! behalf, what counts as correct.

use anyhow::{Result, ensure};
use camino::Utf8PathBuf;

use super::{
    CargoInvocation, FakeRelease, MakeInvocation, RecordingCargo, Sandbox, combined,
    pinned_mold_version, pinned_toolchain,
};

/// The version a fake mold release is published as.
///
/// Deliberately not a real mold version, so a test that accidentally reached
/// the network would fail rather than silently succeed against an upstream
/// artefact.
pub const TEST_MOLD_VERSION: &str = "9.9.9";

/// A well-formed digest that cannot match any artefact.
pub const WRONG_SHA256: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// The installer inputs pointing at a local fake release.
///
/// Grouping them keeps the pin path, checksum path, and URL travelling together
/// as one value instead of as a handful of interchangeable strings.
pub struct InstallerFixture {
    version_pin: Utf8PathBuf,
    checksums: Utf8PathBuf,
    base_url: String,
}

impl InstallerFixture {
    /// Environment overrides for a direct `install-dev-fast.sh` invocation.
    #[must_use]
    pub fn script_env(&self) -> Vec<(&'static str, String)> {
        vec![
            ("MOLD_VERSION_FILE", self.version_pin.to_string()),
            ("MOLD_SHA256SUMS_FILE", self.checksums.to_string()),
            ("MOLD_RELEASE_BASE_URL", self.base_url.clone()),
        ]
    }
}

/// A sandbox with a published fake release and a usable `rustup`.
pub struct InstallerScenario {
    sandbox: Sandbox,
    release: FakeRelease,
}

impl InstallerScenario {
    /// Publish a release and make the Cranelift toolchain appear installed, so
    /// only the linker half of the installer is under test.
    ///
    /// # Errors
    ///
    /// Returns an error if the installer scenario cannot be prepared.
    pub fn prepare() -> Result<Self> {
        let sandbox = Sandbox::new()?;
        sandbox.write_rustup(&pinned_toolchain()?, true)?;
        let release = FakeRelease::publish(&sandbox, TEST_MOLD_VERSION)?;
        Ok(Self { sandbox, release })
    }

    /// The sandbox the scenario runs in.
    #[must_use]
    pub const fn sandbox(&self) -> &Sandbox {
        &self.sandbox
    }

    /// The published release under test.
    #[must_use]
    pub const fn release(&self) -> &FakeRelease {
        &self.release
    }

    /// Where a successful install lands the linker.
    #[must_use]
    pub fn installed_mold(&self) -> Utf8PathBuf {
        self.sandbox.prefix().join("bin/mold")
    }

    /// Installer inputs verified against `checksums`.
    ///
    /// # Errors
    ///
    /// Returns an error if the installer fixture cannot be assembled.
    pub fn fixture(&self, checksums: Utf8PathBuf) -> Result<InstallerFixture> {
        Ok(InstallerFixture {
            version_pin: self.release.write_version_pin(&self.sandbox)?,
            checksums,
            base_url: self.release.base_url(),
        })
    }

    /// Inputs recording the release's real digest, so verification passes.
    ///
    /// # Errors
    ///
    /// Returns an error if the matching checksum fixture cannot be written.
    pub fn with_matching_checksum(&self) -> Result<InstallerFixture> {
        self.fixture(
            self.release
                .write_checksums(&self.sandbox, self.release.sha256())?,
        )
    }
}

/// A sandbox whose prerequisites all pass, with a recording `cargo` installed.
pub struct BuildScenario {
    sandbox: Sandbox,
    cargo: RecordingCargo,
}

impl BuildScenario {
    /// Stage a sandbox in which `dev-fast-check` passes.
    ///
    /// # Errors
    ///
    /// Returns an error if the build scenario cannot be prepared.
    pub fn prepare() -> Result<Self> {
        let sandbox = Sandbox::new()?;
        sandbox.write_mold(&sandbox.prefix().join("bin"), &pinned_mold_version()?)?;
        sandbox.write_rustup(&pinned_toolchain()?, true)?;
        let cargo = RecordingCargo::install(&sandbox)?;
        Ok(Self { sandbox, cargo })
    }

    /// The sandbox the scenario runs in.
    #[must_use]
    pub const fn sandbox(&self) -> &Sandbox {
        &self.sandbox
    }

    /// The recording Cargo standing in for the real one.
    #[must_use]
    pub const fn cargo(&self) -> &RecordingCargo {
        &self.cargo
    }

    /// The install prefix's `bin`, which the `dev-*` recipes lead `PATH` with.
    #[must_use]
    pub fn prefix_bin(&self) -> Utf8PathBuf {
        self.sandbox.prefix().join("bin")
    }

    /// Run `target`, pointing `CARGO` at the recording fake, and return the
    /// single invocation it must have produced.
    ///
    /// # Errors
    ///
    /// Returns an error if the target fails or its Cargo invocation cannot be read.
    pub fn run(&self, target: &str) -> Result<CargoInvocation> {
        let invocation = MakeInvocation::new(target).variable("CARGO", self.cargo.executable());
        let output = self.sandbox.run_make(&invocation)?;
        ensure!(
            output.status.success(),
            "make {target} should succeed, got `{}`",
            combined(&output)
        );
        self.cargo.sole_invocation()
    }
}
