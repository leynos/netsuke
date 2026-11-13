//! Probe the terminal stream from inside a function, which must be rejected.
//!
//! `clippy.toml` disallows `std::io::IsTerminal::is_terminal` so a presentation
//! decision cannot be re-derived from ambient state deep in a call tree. This
//! fixture is the unsanctioned caller: it is compiled with `clippy-driver` and
//! `-D warnings`, and it must fail with the `disallowed_methods` diagnostic.
//!
//! It is a `compile_fail` fixture rather than a test of the workspace's own
//! code because the workspace's only sanctioned site is `main`, which cannot
//! be reached from a test without running the binary.

fn main() {
    let is_terminal = std::io::IsTerminal::is_terminal(&std::io::stdout());
    println!("stdout is a terminal: {is_terminal}");
}
