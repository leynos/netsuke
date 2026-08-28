//! Manifest values that accept either one string or an ordered list.

use serde::{Deserialize, Serialize};

/// A helper for fields that accept either a single string or a list of
/// strings.
///
/// It mirrors YAML syntax where a scalar or sequence is allowed. Empty values
/// deserialize to `StringOrList::Empty`.
///
/// ```yaml
/// # Scalar
/// name: hello
/// # Sequence
/// name:
///   - hello
///   - world
/// ```
#[derive(Debug, Deserialize, Serialize, Default, Clone, PartialEq)]
#[serde(untagged)]
pub enum StringOrList {
    /// No value provided.
    #[default]
    Empty,
    /// A single string item.
    String(String),
    /// A list of string items.
    List(Vec<String>),
}

impl StringOrList {
    /// Apply `f` to each contained string, collecting the results.
    ///
    /// `Empty` yields an empty vector, `String` a single-element vector, and
    /// `List` one element per item.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::ast::StringOrList;
    ///
    /// let single = StringOrList::String("hello".into());
    /// assert_eq!(single.map_each(str::len), vec![5]);
    /// assert!(StringOrList::Empty.map_each(str::len).is_empty());
    /// ```
    #[must_use]
    pub fn map_each<T, F>(&self, f: F) -> Vec<T>
    where
        F: Fn(&str) -> T,
    {
        match self {
            Self::Empty => Vec::new(),
            Self::String(s) => vec![f(s)],
            // Indexed iteration keeps the Kani harnesses in
            // `crate::ir::from_manifest_verification` tractable; an iterator
            // chain here defeats their loop unwinding bounds.
            Self::List(v) => {
                let mut mapped = Vec::with_capacity(v.len());
                let mut index = 0;
                while let Some(value) = v.get(index) {
                    mapped.push(f(value));
                    index += 1;
                }
                mapped
            }
        }
    }

    /// Collect the contained strings into owned `String`s.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::ast::StringOrList;
    ///
    /// let rule = StringOrList::String("cc".into());
    /// assert_eq!(rule.to_string_vec(), vec!["cc".to_owned()]);
    /// ```
    #[must_use]
    pub fn to_string_vec(&self) -> Vec<String> {
        self.map_each(str::to_owned)
    }

    /// Return the sole contained string, if exactly one is present.
    ///
    /// A `String` value or a one-element `List` yields `Some`; anything else
    /// yields `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::ast::StringOrList;
    ///
    /// assert_eq!(StringOrList::String("cc".into()).as_single(), Some("cc"));
    /// assert_eq!(
    ///     StringOrList::List(vec!["a".into(), "b".into()]).as_single(),
    ///     None,
    /// );
    /// ```
    #[must_use]
    pub fn as_single(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            Self::List(v) if v.len() == 1 => v.first().map(String::as_str),
            _ => None,
        }
    }

    /// Whether the value carries no string content.
    ///
    /// `Empty` and an empty `List` both yield `true`; a `String` (even an
    /// empty string) and a non-empty `List` yield `false`.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::ast::StringOrList;
    ///
    /// assert!(StringOrList::Empty.is_empty_content());
    /// assert!(StringOrList::List(Vec::new()).is_empty_content());
    /// assert!(!StringOrList::String(String::new()).is_empty_content());
    /// ```
    #[must_use]
    pub const fn is_empty_content(&self) -> bool {
        match self {
            Self::Empty => true,
            Self::String(_) => false,
            Self::List(v) => v.is_empty(),
        }
    }

    /// Report whether the value has no non-whitespace string content.
    ///
    /// This keeps dependency-only validation narrow: executable recipes retain
    /// their existing empty-string semantics, while blank dependency templates
    /// cannot satisfy the requirement for a real prerequisite.
    ///
    /// For example, a scalar containing only whitespace and a list containing
    /// only empty strings are blank; a list containing `"check"` is not.
    #[must_use]
    pub(crate) fn is_blank_content(&self) -> bool {
        match self {
            Self::Empty => true,
            Self::String(value) => value.trim().is_empty(),
            Self::List(values) => values.iter().all(|value| value.trim().is_empty()),
        }
    }
}

impl From<&str> for StringOrList {
    fn from(value: &str) -> Self {
        Self::String(value.to_owned())
    }
}

impl From<String> for StringOrList {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<Vec<String>> for StringOrList {
    fn from(value: Vec<String>) -> Self {
        Self::List(value)
    }
}
