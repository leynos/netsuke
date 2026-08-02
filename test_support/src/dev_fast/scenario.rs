//! A sandbox with every `dev-fast` prerequisite satisfied.
//!
//! Shared because both the Make-recipe tests and the benchmark tests need the
//! same starting point: a pinned `mold` on the install prefix, a `rustup` that
//! reports the Cranelift component, and a recording Cargo standing in for the
//! real one.

use anyhow::{Result, ensure};
use camino::Utf8PathBuf;

use super::{
    CargoInvocation, MakeInvocation, RecordingCargo, Sandbox, combined, pinned_mold_version,
    pinned_toolchain,
};

/// A sandbox whose prerequisites all pass, with a recording `cargo` installed.
pub struct BuildScenario {
    sandbox: Sandbox,
    cargo: RecordingCargo,
}

impl BuildScenario {
    /// Stage a sandbox in which `dev-fast-check` passes.
    pub fn prepare() -> Result<Self> {
        let sandbox = Sandbox::new()?;
        sandbox.write_mold(&sandbox.prefix().join("bin"), &pinned_mold_version()?)?;
        sandbox.write_rustup(&pinned_toolchain()?, true)?;
        let cargo = RecordingCargo::install(&sandbox)?;
        Ok(Self { sandbox, cargo })
    }

    /// The sandbox the scenario runs in.
    pub fn sandbox(&self) -> &Sandbox {
        &self.sandbox
    }

    /// The recording Cargo standing in for the real one.
    pub fn cargo(&self) -> &RecordingCargo {
        &self.cargo
    }

    /// The install prefix's `bin`, which the `dev-*` recipes lead `PATH` with.
    pub fn prefix_bin(&self) -> Utf8PathBuf {
        self.sandbox.prefix().join("bin")
    }

    /// Run `target`, pointing `CARGO` at the recording fake, and return the
    /// single invocation it must have produced.
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
