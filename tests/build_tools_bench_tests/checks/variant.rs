//! One variant's recorded pair of builds, and the contract its row must meet.
//!
//! Split from the parent module to keep each readable: the parent checks the
//! shape of a whole run, and this checks a single variant within it.

use anyhow::{Context, Result, ensure};
use test_support::build_tools::{CargoInvocation, TargetState};

/// One variant's pair of recorded builds and the contract its row must meet.
///
/// The flags are the descriptor's whole point: they are the only thing that
/// distinguishes one benchmark row from another, so a variant whose flags
/// drifted is the whole of the bug this can have.
pub struct BenchVariant<'a> {
    pub label: &'a str,
    clean: &'a CargoInvocation,
    incremental: &'a CargoInvocation,
    /// The flags this variant must hand Cargo, which is the only thing that
    /// distinguishes one row from another.
    pub flags: &'a [&'a str],
}

impl<'a> BenchVariant<'a> {
    /// Build a descriptor from the variant's recorded pair of passes.
    ///
    /// The fixed-size array makes the pairing a type-level fact, so there is no
    /// absent-pass case left to handle at runtime.
    pub const fn from_pair(
        label: &'a str,
        [clean, incremental]: &'a [CargoInvocation; 2],
        flags: &'a [&'a str],
    ) -> Self {
        Self {
            label,
            clean,
            incremental,
            flags,
        }
    }

    pub fn target_dir(&self) -> &str {
        self.clean.target_dir()
    }

    pub fn check(&self, toolchain: &str) -> Result<()> {
        let label = self.label;
        for pass in [self.clean, self.incremental] {
            ensure!(
                pass.contains_sequence(&["build", "--bin", "netsuke"]),
                "`{label}` should build the binary, got `{:?}`",
                pass.arguments()
            );
            // Assigned, never inherited: an unset RUSTFLAGS would let
            // `.cargo/config.toml` decide the variant, and every row would
            // then measure the same build.
            let recorded: Vec<&str> = pass
                .rustflags()
                .with_context(|| format!("`{label}` must assign RUSTFLAGS"))?
                .split_whitespace()
                .collect();
            ensure!(
                recorded == self.flags,
                "`{label}` should pass {:?}, got {recorded:?}",
                self.flags
            );
            ensure!(
                pass.toolchain() == toolchain,
                "`{label}` should run under `{toolchain}`, got `{}`",
                pass.toolchain()
            );
            // Both compiler wrappers assigned and empty, for the same reason
            // RUSTFLAGS is assigned rather than inherited. A developer shell
            // on a shared host commonly exports a wrapper chaining to
            // `sccache`; with one in force the first clean pass fills the
            // cache and the rest read it back, so the table times cache
            // retrieval and the row order decides the winner. Unset is not
            // good enough as an expectation: only an assignment displaces an
            // exported value, and `RUSTC_WORKSPACE_WRAPPER` is honoured
            // separately, so clearing one alone still wraps the workspace's
            // own crates.
            ensure!(
                pass.wrappers_cleared(),
                "`{label}` should clear both compiler wrappers, got RUSTC_WRAPPER={:?} \
                 RUSTC_WORKSPACE_WRAPPER={:?}",
                pass.wrapper(),
                pass.workspace_wrapper()
            );
            // Unset, not merely empty. Cargo reads the encoded variable before
            // `RUSTFLAGS`, so an empty one would still displace every variant's
            // flags — the rows would all compile identically while the table
            // reported three different builds.
            ensure!(
                pass.encoded_rustflags().is_none(),
                "`{label}` must remove CARGO_ENCODED_RUSTFLAGS, found {:?}",
                pass.encoded_rustflags()
            );
        }

        ensure!(
            self.clean.target_dir() == self.incremental.target_dir(),
            "`{label}` should reuse one target directory across its passes"
        );
        ensure!(
            self.clean.target_dir().ends_with(label),
            "`{label}` should measure in its own directory, got `{}`",
            self.clean.target_dir()
        );

        // The harness removes the directory before the clean pass, and the
        // clean pass leaves it behind, so this ordering is what distinguishes
        // a genuine clean/incremental pair from two identical builds.
        ensure!(
            self.clean.target_state() == TargetState::Absent,
            "`{label}` clean pass should start from an empty target directory, got {:?}",
            self.clean.target_state()
        );
        ensure!(
            self.incremental.target_state() == TargetState::Present,
            "`{label}` incremental pass should reuse the clean pass's output, got {:?}",
            self.incremental.target_state()
        );

        Ok(())
    }
}
