//! Ninja runner content and CLI-to-process adapters.

mod content;
mod process_adapter;

pub use content::NinjaContent;
pub(super) use process_adapter::ninja_process_options;
pub use process_adapter::{run_ninja, run_ninja_tool};
