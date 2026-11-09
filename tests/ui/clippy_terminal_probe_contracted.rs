//! Probe the terminal stream under an explicit, reasoned exception.
//!
//! This is the control half of `clippy_terminal_probe_uncontracted.rs`: the
//! same denied call, this time carrying the `#[expect]` the workspace's own
//! entrypoint uses. Clippy must accept it, which is what shows the rejection
//! test beside this one is reacting to the denial rather than to a broken
//! invocation.
//!
//! `expect` rather than `allow` is deliberate: it fails to compile if the lint
//! ever stops firing here, so this fixture cannot quietly become a no-op.

/// Report whether standard output is a terminal.
#[expect(
    clippy::disallowed_methods,
    reason = "compile-fail control fixture: proves the sanctioned exception shape is accepted"
)]
fn stdout_is_terminal() -> bool {
    std::io::IsTerminal::is_terminal(&std::io::stdout())
}

fn main() {
    println!("stdout is a terminal: {}", stdout_is_terminal());
}
