//! Typed conversion from shell text to a safe Ninja binding value.

use std::fmt::{self, Display, Formatter};

use super::NinjaGenError;

/// Fully assembled POSIX shell text without Ninja-specific escaping.
pub(super) struct ShellText(String);

impl ShellText {
    /// Construct shell text before the Ninja backend serializes it.
    pub(super) const fn new(text: String) -> Self {
        Self(text)
    }
}

/// Text safe to emit on the right-hand side of a Ninja binding.
///
/// Only [`escape_ninja_value`] constructs this type, so a command crosses the
/// backend escaping boundary exactly once.
pub(super) struct NinjaValue(String);

impl Display for NinjaValue {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Escape fully assembled shell text for one Ninja binding.
///
/// Literal dollars become `$$`. Control characters are rejected because they
/// could add a new Ninja statement instead of remaining part of the binding.
pub(super) fn escape_ninja_value(text: &ShellText) -> Result<NinjaValue, NinjaGenError> {
    if text.0.contains(['\n', '\r', '\0']) {
        return Err(NinjaGenError::UnsafeNinjaValue);
    }
    Ok(NinjaValue(text.0.replace('$', "$$")))
}
