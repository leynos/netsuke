//! Resolve the Ninja executable and interpret its process status output.

mod program;
mod status;

pub use program::resolve_ninja_program;
// Streaming consumes parsed progress through the process boundary only.
pub(super) use status::{NinjaTaskProgressTracker, parse_ninja_status_line};
