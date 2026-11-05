//! Validate the `args` field of a trusted shell definition.
//!
//! An argument vector is passed to a program unchanged, so it is checked for the
//! shape a process can carry: at least one element, no more than sixteen, no
//! element containing NUL, and a total UTF-8 size within a fixed budget. Empty
//! elements are permitted because they are meaningful to some programs.
//!
//! The budget is spent by subtraction rather than by summing lengths and
//! comparing afterwards. Summing a caller-controlled number of lengths can
//! overflow, and a saturating sum would silently accept an oversized vector,
//! whereas a budget that runs out fails exactly once and reports the limit.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use thiserror::Error;

/// Set the largest number of arguments a definition may declare.
pub const MAX_ARGUMENTS: usize = 16;

/// Set the largest UTF-8 byte total a definition's arguments may declare.
pub const MAX_ARGUMENT_BYTES: usize = 4096;

/// Retain an accepted argument vector exactly as it was configured.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShellArguments(Vec<String>);

impl ShellArguments {
    /// Validate `arguments` as a complete argument vector.
    ///
    /// # Errors
    ///
    /// Returns [`ArgumentsError::Empty`] for no arguments,
    /// [`ArgumentsError::TooMany`] above [`MAX_ARGUMENTS`],
    /// [`ArgumentsError::Nul`] naming the first offending position, and
    /// [`ArgumentsError::TooLong`] once the byte budget is exhausted.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::cli::config::shells::ShellArguments;
    /// let vector = vec!["-NoLogo".to_owned(), String::new()];
    /// assert!(ShellArguments::parse(&vector).is_ok());
    /// assert!(ShellArguments::parse(&[]).is_err());
    /// ```
    pub fn parse(arguments: &[String]) -> Result<Self, ArgumentsError> {
        if arguments.is_empty() {
            return Err(ArgumentsError::Empty);
        }
        if arguments.len() > MAX_ARGUMENTS {
            return Err(ArgumentsError::TooMany {
                count: arguments.len(),
            });
        }
        check_elements(arguments)?;
        Ok(Self(arguments.to_vec()))
    }

    /// Borrow the accepted argument vector in configured order.
    #[must_use]
    pub const fn as_slice(&self) -> &[String] {
        self.0.as_slice()
    }
}

impl fmt::Display for ShellArguments {
    /// Render the argument count rather than the arguments themselves.
    ///
    /// A diagnostic that echoed a full argument vector could disclose an
    /// operator's secrets, so only the count is ever displayed.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} argument(s)", self.0.len())
    }
}

impl Serialize for ShellArguments {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ShellArguments {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let arguments = Vec::<String>::deserialize(deserializer)?;
        Self::parse(&arguments).map_err(serde::de::Error::custom)
    }
}

/// Reject an element containing NUL, then reject a vector over the byte budget.
///
/// The NUL scan runs first so a vector that is both over budget and impossible
/// to pass to a program reports the impossible element, which is the more
/// actionable fault.
fn check_elements(arguments: &[String]) -> Result<(), ArgumentsError> {
    if let Some(index) = arguments.iter().position(|text| text.contains('\0')) {
        return Err(ArgumentsError::Nul { index });
    }
    spend_budget(arguments)
}

/// Subtract every element's length from the budget, failing when it runs out.
fn spend_budget(arguments: &[String]) -> Result<(), ArgumentsError> {
    let mut remaining = MAX_ARGUMENT_BYTES;
    for text in arguments {
        remaining = remaining
            .checked_sub(text.len())
            .ok_or(ArgumentsError::TooLong)?;
    }
    Ok(())
}

/// Classify the argument-vector rules a definition may violate.
///
/// Each variant names the rule and enough structure to locate it, never the
/// argument text: the caller decides how much of that to quote.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum ArgumentsError {
    /// Reject a definition that declares no arguments at all.
    #[error("a shell definition must declare at least one argument")]
    Empty,
    /// Reject a vector above the element limit.
    #[error(
        "a shell definition must not declare more than {MAX_ARGUMENTS} arguments, \
         but declared {count}"
    )]
    TooMany {
        /// Report how many arguments the definition declared.
        count: usize,
    },
    /// Reject an element a process cannot carry.
    #[error("argument {index} must not contain a NUL character")]
    Nul {
        /// Report the zero-based position of the offending element.
        index: usize,
    },
    /// Reject a vector that exceeds the byte budget.
    #[error("a shell definition's arguments must total at most {MAX_ARGUMENT_BYTES} bytes")]
    TooLong,
}
