//! Test scaffolding for the opt-in `dev-fast` build-acceleration tooling.
//!
//! The `make dev-*` targets and their backing scripts probe `PATH` for `mold`
//! and `rustup`, download a pinned release, and shell out to Cargo. Testing
//! them needs three things this module provides:
//!
//! - [`Sandbox`], a `PATH` and `HOME` built from nothing, so a case can express
//!   "the tool is absent" on a machine that has the real tool installed.
//! - [`FakeRelease`], a locally built stand-in for a mold release served over a
//!   `file://` URL, so the installer's download and verification path runs
//!   without a network.
//! - [`RecordingCargo`], a fake `cargo` that records the arguments and
//!   environment each invocation received, so a target's command line can be
//!   asserted rather than inferred.
//!
//! [`MakeInvocation`] ties them together, describing a Make run with its
//! variable overrides and environment so tests exercise the real recipes rather
//! than reimplementing them.
//!
//! Scope: these helpers exist for the `dev-fast` target tests. They spawn child
//! processes with a bespoke environment and never mutate the parent's, so they
//! qualify for the subprocess-isolation exemption to the ban on in-process
//! environment mutation (`AGENTS.md`). That exemption is written around
//! `assert_cmd`; these helpers use `std::process::Command` directly, because
//! the subject under test is `make` and the `scripts/` entry points rather than
//! the `netsuke` binary. The isolation is the same, and stricter: `env_clear`
//! means a child inherits nothing it was not handed.

mod bench;
mod cargo_log;
mod make;
mod release;
mod sandbox;
mod scenario;
mod staging;

pub use bench::{BASELINE_MTIME, BenchFixture, DEFAULT_SLUG, DEV_FAST_SLUG, write_with_old_mtime};
pub use cargo_log::{CargoInvocation, RecordingCargo, TargetState};
pub use make::MakeInvocation;
pub use release::FakeRelease;
pub use sandbox::{
    DEV_FAST_CONFIG_PATH, PinOverrides, Sandbox, combined, dev_fast_config, pinned_mold_version,
    pinned_toolchain,
};
pub use scenario::{
    BuildScenario, InstallerFixture, InstallerScenario, TEST_MOLD_VERSION, WRONG_SHA256,
};
