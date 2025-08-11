use std::ffi::OsString;

/// Guard that restores `PATH` to its original value when dropped.
///
/// This uses RAII to ensure the environment is reset even if a test panics.
#[allow(
    unfulfilled_lint_expectations,
    reason = "used only in select test crates",
)]
#[expect(dead_code, reason = "constructed only in PATH tests")]
#[derive(Debug)]
pub struct PathGuard {
    original: OsString,
}

#[allow(
    unfulfilled_lint_expectations,
    reason = "used only in select test crates",
)]
impl PathGuard {
    /// Create a guard capturing the current `PATH`.
    #[expect(dead_code, reason = "constructed only in PATH tests")]
    pub fn new(original: OsString) -> Self {
        Self { original }
    }
}

impl Drop for PathGuard {
    fn drop(&mut self) {
        // Nightly marks `set_var` unsafe; restoring `PATH` cleans up global state.
        unsafe { std::env::set_var("PATH", &self.original) };
    }
}
