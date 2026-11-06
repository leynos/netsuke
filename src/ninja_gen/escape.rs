//! Typed conversion from shell text to a safe Ninja binding value.

use std::fmt::{self, Display, Formatter};

use super::NinjaGenError;

/// Fully assembled recipe text without Ninja-specific escaping.
pub(super) struct ShellText(String);

impl ShellText {
    /// Construct shell text before the Ninja backend serializes it.
    pub(super) const fn new(text: String) -> Self {
        Self(text)
    }

    /// Borrow the completed recipe text before Ninja serialization.
    pub(super) fn as_str(&self) -> &str {
        &self.0
    }
}

/// Text safe to emit on the right-hand side of a Ninja binding.
///
/// Renderers use [`escape_ninja_value`] for the whole value, except when the
/// value must retain a Ninja variable reference such as `$rspfile`. In that
/// case the dynamic portion is escaped first and the assembled text is marked
/// with [`NinjaValue::from_encoded`].
pub(super) struct NinjaValue(String);

impl NinjaValue {
    /// Wrap text whose dynamic portions are already escaped.
    pub(super) const fn from_encoded(value: String) -> Self {
        Self(value)
    }
}

impl Display for NinjaValue {
    /// Render the already escaped binding text without another conversion.
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Reject text that cannot remain within one Ninja binding.
///
/// The predicate itself lives in `shell_word` so the Ninja writer and the
/// recipe-text template filters share one definition rather than a copy at
/// each enforcement point; this wrapper only supplies the error type.
pub(super) fn validate_ninja_value(text: &str) -> Result<(), NinjaGenError> {
    if crate::shell_word::is_recipe_admissible(text) {
        return Ok(());
    }
    Err(NinjaGenError::UnsafeNinjaValue)
}

/// Escape fully assembled shell text for one Ninja binding.
///
/// Literal dollars become `$$`. Control characters are rejected because they
/// could add a new Ninja statement instead of remaining part of the binding.
pub(super) fn escape_ninja_value(text: &ShellText) -> Result<NinjaValue, NinjaGenError> {
    validate_ninja_value(text.as_str())?;
    Ok(NinjaValue(text.as_str().replace('$', "$$")))
}

/// Escape an optional metadata value for one Ninja binding.
pub(super) fn escape_metadata_value(
    value: Option<&str>,
) -> Result<Option<NinjaValue>, NinjaGenError> {
    value
        .map(|metadata| {
            validate_ninja_value(metadata)?;
            Ok(NinjaValue(metadata.replace('$', "$$")))
        })
        .transpose()
}
