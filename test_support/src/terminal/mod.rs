//! Capture terminal output and classify control sequences in integration tests.
//!
//! This test-only interface is shared by integration tests and BDD steps. It
//! has no production callers; byte classification is available on every
//! platform, while pseudo-terminal execution is Unix-only.

mod classify;
pub use classify::{TerminalBytes, classify};

#[cfg(unix)]
mod pty;
#[cfg(unix)]
pub use pty::{Attach, PtyOutput, PtyRequest, run_in_pty};
