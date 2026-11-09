//! Locate the installed verifier the isolation case should exercise.
//!
//! The boundary case builds everything it needs except the verifier itself: the
//! lane installs `cargo-kani` and names it through an environment variable, and
//! the case has to resolve both the launcher and the payload it loads before the
//! sandbox can bind them. Keeping the lookup here leaves [`super`] to the
//! fixture plumbing and [`super::sandbox`] to the scripts.

use anyhow::{Context, Result, ensure};
use std::path::{Path, PathBuf};

/// Environment variable naming an installed verifier to exercise in the sandbox.
///
/// The isolation case can only use a verifier it can reach, and the fixture's
/// stub is what makes a clean trace and a known-dirty trace possible. CI sets
/// this to the pinned `cargo-kani` it installed, so the boundary also runs the
/// real executable through the same recipe. Unset, the case exercises the stub
/// alone, which is what a host without Kani can still do.
const ISOLATION_VERIFIER_ENV: &str = "NETSUKE_KANI_ISOLATION_VERIFIER";

/// An installed verifier and the payload it resolves, both as host paths.
///
/// `cargo-kani` is a launcher as well as a driver: asked for its version it
/// first locates a payload under the Kani home, and only a home it can read
/// keeps it from starting a first-time setup. The sandbox therefore exposes the
/// binary and the home together, so the query under test is the version query
/// rather than an installer reaching for the network.
pub(crate) struct InstalledKani {
    pub(crate) binary: PathBuf,
    pub(crate) home: PathBuf,
}

impl InstalledKani {
    /// Return the directory the sandbox must bind to expose the binary.
    ///
    /// A path with no parent is bound from itself, which the caller's
    /// validation has already ruled out; the fallback keeps the expression
    /// total rather than panicking on a shape that cannot arrive.
    pub(crate) fn directory(&self) -> &Path {
        self.binary.parent().unwrap_or(&self.binary)
    }
}

/// Return the installed verifier the boundary should exercise, if named.
///
/// This reads the caller's environment rather than a fixture, so it is a free
/// function: there is no state to consult, and taking a receiver would imply
/// otherwise.
///
/// The variables are read here rather than inside the sandbox because the
/// capability probe needs the same paths: a host that can build the chroot but
/// cannot bind either directory must report the prerequisite, not fail later
/// inside the script.
///
/// # Errors
///
/// Returns an error if the variable names a path that cannot be resolved or is
/// not a file, or if no Kani home exists, so a lane that means to exercise the
/// installed binary fails rather than silently running the stub alone.
#[expect(
    clippy::disallowed_methods,
    reason = "this is the single site that reads the install and payload \
              paths; the test binary has no injected environment to consult"
)]
pub(crate) fn installed_verifier() -> Result<Option<InstalledKani>> {
    let Some(value) = std::env::var_os(ISOLATION_VERIFIER_ENV) else {
        return Ok(None);
    };
    let path = PathBuf::from(value);
    let binary = path
        .canonicalize()
        .with_context(|| format!("resolve the installed verifier at {}", path.display()))?;
    ensure!(
        binary.is_file(),
        "{ISOLATION_VERIFIER_ENV} must name an executable file, got {}",
        binary.display()
    );
    // The launcher prefers `KANI_HOME` and falls back to `$HOME/.kani`;
    // resolving the same way is what makes the bound payload the one it looks
    // for.
    let home = std::env::var_os("KANI_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".kani")))
        .context("locate the Kani home from KANI_HOME or HOME")?;
    ensure!(
        home.is_dir(),
        "the Kani home {} must exist so the launcher does not start a setup",
        home.display()
    );
    Ok(Some(InstalledKani {
        binary,
        home: home
            .canonicalize()
            .with_context(|| format!("resolve the Kani home at {}", home.display()))?,
    }))
}
