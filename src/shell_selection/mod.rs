//! Describe structured-command shell intent without resolving or executing it.
//!
//! Names are shared with future trusted configuration; built-ins describe fixed
//! invocations. Keep environment and executable resolution outside this module.

mod built_in;
mod name;

pub use built_in::{BuiltInExecutable, BuiltInShell, HostFamily};
pub use name::{ShellName, ShellNameError};
