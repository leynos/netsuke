//! Encode and decode the manifest-facing shell intent vocabulary.

use super::ShellName;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::fmt;

#[cfg(test)]
mod property_tests;
#[cfg(test)]
mod tests;

/// Represent direct execution, a host default, or a validated shell name.
///
/// # Examples
///
/// ```
/// use std::collections::HashSet;
///
/// use netsuke::shell_selection::{ShellName, ShellSelection};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let platform_default: ShellSelection = serde_json::from_str("true")?;
/// let named: ShellSelection = serde_json::from_str(r#""bash""#)?;
/// assert_eq!(platform_default, ShellSelection::PlatformDefault);
/// assert_eq!(named, ShellSelection::Named(ShellName::parse("bash")?));
/// assert_eq!(serde_json::to_string(&named)?, r#""bash""#);
///
/// let selections = HashSet::from([platform_default, named]);
/// assert_eq!(selections.len(), 2);
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum ShellSelection {
    /// Run the command directly without selecting a shell.
    #[default]
    Direct,
    /// Resolve the default shell for the target host family.
    PlatformDefault,
    /// Retain an explicitly selected validated shell name.
    Named(
        /// Keep the exact name without resolving or executing it.
        ShellName,
    ),
}

/// Decode only the two Boolean intents and validated shell names.
struct ShellSelectionVisitor;

impl de::Visitor<'_> for ShellSelectionVisitor {
    type Value = ShellSelection;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a Boolean or a shell name")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(if value {
            ShellSelection::PlatformDefault
        } else {
            ShellSelection::Direct
        })
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        ShellName::parse(value)
            .map(ShellSelection::Named)
            .map_err(de::Error::custom)
    }
}

impl Serialize for ShellSelection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Direct => serializer.serialize_bool(false),
            Self::PlatformDefault => serializer.serialize_bool(true),
            Self::Named(name) => serializer.serialize_str(name.as_str()),
        }
    }
}

impl<'de> Deserialize<'de> for ShellSelection {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(ShellSelectionVisitor)
    }
}
