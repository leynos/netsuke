//! Validate shell registry names independently of registry or adapter modules.
//!
//! Keep imports limited to standard and serialization crates so trusted
//! configuration can include this file in the build-script slice later.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{fmt, str::FromStr};
use thiserror::Error;

/// Exclude names that YAML would infer as Booleans when unquoted.
const RESERVED_WORDS: [&str; 8] = ["true", "false", "yes", "no", "y", "n", "on", "off"];

/// Retain a validated `[a-z][a-z0-9_-]{0,62}` registry name.
///
/// YAML Boolean spellings are reserved even when quoted. Configured names need
/// not be built-ins, and parsing performs no executable lookup.
///
/// # Examples
///
/// ```
/// use netsuke::shell_selection::{BuiltInShell, ShellName, ShellNameError};
/// let name = ShellName::parse("dash")?;
/// assert_eq!(name.as_str(), "dash");
/// assert_eq!(BuiltInShell::lookup(&name), None);
/// assert_eq!(ShellName::parse("/bin/bash"),
///     Err(ShellNameError::InvalidFirstCharacter { character: '/' }));
/// assert_eq!(ShellName::parse("true"),
///     Err(ShellNameError::ReservedWord { word: "true" }));
/// # Ok::<(), ShellNameError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ShellName(
    /// Own the validated spelling; only `BuiltInShell`'s `From` implementation
    /// may construct this field unchecked, as its finite names are tested.
    /// This visibility avoids a spare inherent method in the build-script slice.
    pub(super) String,
);

impl ShellName {
    /// Limit names to 63 ASCII bytes.
    pub const MAX_LEN: usize = 63;

    /// Parse an exact shell name without trimming or case folding.
    ///
    /// # Errors
    ///
    /// Reject empty input, excessive UTF-8 byte length, invalid first or later
    /// characters, then reserved Boolean spellings, in that precedence order.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::shell_selection::ShellName;
    /// assert_eq!(ShellName::parse("dash")?.as_str(), "dash");
    /// # Ok::<(), netsuke::shell_selection::ShellNameError>(())
    /// ```
    pub fn parse(text: &str) -> Result<Self, ShellNameError> {
        if text.is_empty() {
            return Err(ShellNameError::Empty);
        }
        if text.len() > Self::MAX_LEN {
            return Err(ShellNameError::TooLong { length: text.len() });
        }
        let mut characters = text.chars().enumerate();
        let (_, first) = characters.next().ok_or(ShellNameError::Empty)?;
        if !first.is_ascii_lowercase() {
            return Err(ShellNameError::InvalidFirstCharacter { character: first });
        }
        if let Some((char_index, character)) = characters
            .find(|(_, character)| !matches!(character, 'a'..='z' | '0'..='9' | '_' | '-'))
        {
            return Err(ShellNameError::InvalidCharacter {
                character,
                char_index,
            });
        }
        if let Some(word) = RESERVED_WORDS.into_iter().find(|word| *word == text) {
            return Err(ShellNameError::ReservedWord { word });
        }
        Ok(Self(text.to_owned()))
    }

    /// Borrow the exact validated spelling.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::shell_selection::ShellName;
    /// assert_eq!(ShellName::parse("dash")?.as_str(), "dash");
    /// # Ok::<(), netsuke::shell_selection::ShellNameError>(())
    /// ```
    #[must_use]
    pub const fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Consume the name and return its validated spelling.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::shell_selection::ShellName;
    /// assert_eq!(ShellName::parse("dash")?.into_inner(), "dash");
    /// # Ok::<(), netsuke::shell_selection::ShellNameError>(())
    /// ```
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl TryFrom<&str> for ShellName {
    type Error = ShellNameError;
    fn try_from(text: &str) -> Result<Self, Self::Error> {
        Self::parse(text)
    }
}

impl TryFrom<String> for ShellName {
    type Error = ShellNameError;
    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::parse(&text)
    }
}

impl FromStr for ShellName {
    type Err = ShellNameError;
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text)
    }
}

impl AsRef<str> for ShellName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for ShellName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Serialize for ShellName {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ShellName {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(serde::de::Error::custom)
    }
}

/// Classify bounded shell-name failures without retaining the whole input.
///
/// # Examples
///
/// ```
/// use netsuke::shell_selection::{ShellName, ShellNameError};
/// assert_eq!(ShellName::parse(""), Err(ShellNameError::Empty));
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum ShellNameError {
    /// Reject an empty spelling.
    #[error("a shell name must not be empty")]
    Empty,
    /// Reject a spelling exceeding the byte limit.
    #[error("a shell name must be at most 63 bytes, but has {length}")]
    TooLong {
        /// Count the original UTF-8 bytes.
        length: usize,
    },
    /// Reject a first character outside lower-case ASCII letters.
    #[error("a shell name must start with a lower-case ASCII letter, not {character:?}")]
    InvalidFirstCharacter {
        /// Identify the offending character.
        character: char,
    },
    /// Reject a later character outside the registry grammar.
    #[error("invalid shell name character {character:?} at character {char_index}")]
    InvalidCharacter {
        /// Identify the offending character.
        character: char,
        /// Count characters from zero, rather than UTF-8 bytes.
        char_index: usize,
    },
    /// Reject a grammatical spelling reserved by YAML Boolean inference.
    #[error("shell name {word:?} is a reserved YAML Boolean; use an unquoted Boolean instead")]
    ReservedWord {
        /// Borrow the matching reserved constant, keeping diagnostics bounded.
        word: &'static str,
    },
}

#[cfg(test)]
pub(super) mod property_tests;
#[cfg(test)]
mod tests;
