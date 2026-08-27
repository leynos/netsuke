//! Compile-pass fixture for verbose timing reporter construction.
//!
//! Compiled by `tests/command_env_ui_tests.rs` against the built `netsuke`
//! rlib. It proves both the default stderr constructor and the public generic
//! writer constructor are available to an external embedder.

use netsuke::output_prefs::resolve_with;
use netsuke::status::{SilentReporter, VerboseTimingReporter};

fn main() {
    let prefs = resolve_with(None, |_| None);
    let _: VerboseTimingReporter = VerboseTimingReporter::new(Box::new(SilentReporter), prefs);
    let _: VerboseTimingReporter<Vec<u8>> =
        VerboseTimingReporter::with_writer(Box::new(SilentReporter), prefs, Vec::new());
}
