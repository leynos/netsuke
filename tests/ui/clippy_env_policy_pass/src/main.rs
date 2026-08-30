//! Clippy policy pass fixture.
//!
//! The environment-mutation mandate bans in-process `std::env` mutation
//! (`set_var`, `remove_var`, `set_current_dir`). Child-process configuration
//! stays available through the `Command` builders, which this fixture
//! exercises. The harness compiles this crate with
//! `-D clippy::disallowed_methods`; a clean compile proves the sanctioned
//! builder surface is unaffected by the disallow-list.

use std::path::Path;
use std::process::Command;

fn main() {
    let mut command = Command::new("netsuke");
    command
        .current_dir(Path::new("/work"))
        .env("NETSUKE_JOBS", "4")
        .env_clear();
}
