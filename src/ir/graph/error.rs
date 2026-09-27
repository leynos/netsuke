//! Error types for intermediate-representation construction.
//!
//! Keeps recoverable validation failures separate from the graph containers so
//! callers can inspect stable IR diagnostics without coupling to storage.

use crate::localization::LocalizedMessage;
use camino::Utf8PathBuf;
use thiserror::Error;

/// Errors produced during IR generation.
///
/// Each variant documents a distinct validation failure encountered while
/// constructing the intermediate representation from a manifest.
///
/// # Examples
///
/// ```
/// use netsuke::ir::IrGenError;
/// use netsuke::localization::{self, keys};
/// use serde::ser::Error as _;
///
/// fn describe(err: IrGenError) -> String {
///     match err {
///         IrGenError::EmptyRule { target_name, .. } => {
///             format!("{target_name} missing rule")
///         },
///         other => other.to_string(),
///     }
/// }
///
/// assert_eq!(
///     describe(IrGenError::EmptyRule {
///         target_name: "app".into(),
///         message: localization::message(keys::IR_EMPTY_RULE),
///     }),
///     "app missing rule"
/// );
/// ```
#[derive(Debug, Error)]
pub enum IrGenError {
    /// Raised when a directly deserialized manifest violates recipe rules.
    #[error("{message}")]
    InvalidManifest {
        /// Stable schema diagnostic identifying the violated recipe rule.
        message: &'static str,
    },

    /// Raised when a target references a rule that is not defined in the
    /// manifest.
    ///
    /// ```
    /// use netsuke::ir::IrGenError;
    /// use netsuke::localization::{self, keys};
    /// use serde::ser::Error as _;
    ///
    /// let err = IrGenError::RuleNotFound {
    ///     target_name: "app".into(),
    ///     rule_name: "compile".into(),
    ///     message: localization::message(keys::IR_RULE_NOT_FOUND),
    /// };
    /// assert!(matches!(
    ///     err,
    ///     IrGenError::RuleNotFound { rule_name, .. }
    ///         if rule_name == "compile"
    /// ));
    /// ```
    #[error("{message}")]
    RuleNotFound {
        /// Name of the target referencing the missing rule.
        target_name: String,
        /// Rule identifier that was not declared.
        rule_name: String,
        /// Localized error message.
        message: LocalizedMessage,
    },

    /// Triggered when multiple rule names are supplied for a single target.
    ///
    /// ```
    /// use netsuke::ir::IrGenError;
    /// use netsuke::localization::{self, keys};
    /// use serde::ser::Error as _;
    ///
    /// let err = IrGenError::MultipleRules {
    ///     target_name: "lib".into(),
    ///     rules: vec!["c".into(), "cpp".into()],
    ///     message: localization::message(keys::IR_MULTIPLE_RULES),
    /// };
    /// if let IrGenError::MultipleRules { rules, .. } = err {
    ///     assert_eq!(
    ///         rules,
    ///         vec!["c".to_owned(), "cpp".to_owned()]
    ///     );
    /// }
    /// ```
    #[error("{message}")]
    MultipleRules {
        /// Name of the target that specified conflicting rules.
        target_name: String,
        /// Set of rule identifiers provided simultaneously.
        rules: Vec<String>,
        /// Localized error message.
        message: LocalizedMessage,
    },

    /// Returned when a target declares no rule at all.
    ///
    /// ```
    /// use netsuke::ir::IrGenError;
    /// use netsuke::localization::{self, keys};
    /// use serde::ser::Error as _;
    ///
    /// let err = IrGenError::EmptyRule {
    ///     target_name: "docs".into(),
    ///     message: localization::message(keys::IR_EMPTY_RULE).with_arg("target", "docs"),
    /// };
    /// if let IrGenError::EmptyRule { target_name, .. } = err {
    ///     assert_eq!(target_name, "docs");
    /// }
    /// ```
    #[error("{message}")]
    EmptyRule {
        /// Target lacking an associated rule.
        target_name: String,
        /// Localized error message.
        message: LocalizedMessage,
    },

    /// Indicates that more than one build edge produces the same output file.
    ///
    /// ```
    /// use netsuke::ir::IrGenError;
    /// use netsuke::localization::{self, keys};
    /// use serde::ser::Error as _;
    ///
    /// let err = IrGenError::DuplicateOutput {
    ///     outputs: vec!["obj.o".into()],
    ///     message: localization::message(keys::IR_DUPLICATE_OUTPUTS),
    /// };
    /// if let IrGenError::DuplicateOutput { outputs, .. } = err {
    ///     assert_eq!(
    ///         outputs,
    ///         vec!["obj.o".to_owned()]
    ///     );
    /// }
    /// ```
    #[error("{message}")]
    DuplicateOutput {
        /// Outputs produced by more than one build edge.
        outputs: Vec<String>,
        /// Localized error message.
        message: LocalizedMessage,
    },

    /// Emitted when a cycle exists in the target graph.
    ///
    /// ```
    /// use camino::Utf8PathBuf;
    /// use netsuke::ir::IrGenError;
    /// use netsuke::localization::{self, keys};
    /// use serde::ser::Error as _;
    ///
    /// let err = IrGenError::CircularDependency {
    ///     cycle: vec![Utf8PathBuf::from("a"), Utf8PathBuf::from("a")],
    ///     missing_dependencies: Vec::new(),
    ///     message: localization::message(keys::IR_CIRCULAR_DEPENDENCY),
    /// };
    /// if let IrGenError::CircularDependency { cycle, .. } = err {
    ///     assert_eq!(
    ///         cycle,
    ///         vec![Utf8PathBuf::from("a"), Utf8PathBuf::from("a")]
    ///     );
    /// }
    /// ```
    #[error("{message}")]
    CircularDependency {
        /// Sequence of outputs that forms the dependency cycle.
        cycle: Vec<Utf8PathBuf>,
        /// Dependencies that could not be resolved during analysis.
        missing_dependencies: Vec<(Utf8PathBuf, Utf8PathBuf)>,
        /// Localized error message.
        message: LocalizedMessage,
    },

    /// Wraps failures encountered while serialising an action to JSON.
    ///
    /// ```
    /// use netsuke::ir::IrGenError;
    /// use netsuke::localization::{self, keys};
    /// use serde::ser::Error as _;
    ///
    /// let source = serde_json::Error::custom("invalid action");
    /// let err = IrGenError::ActionSerialisation {
    ///     source,
    ///     message: localization::message(keys::IR_ACTION_SERIALISATION),
    /// };
    /// assert!(err.to_string().contains("invalid action"));
    /// ```
    #[error("{message}: {source}")]
    ActionSerialisation {
        /// Underlying serialisation error.
        #[source]
        source: serde_json::Error,
        /// Localized error message.
        message: LocalizedMessage,
    },

    /// Raised when command interpolation yields an invalid shell snippet.
    ///
    /// ```
    /// use netsuke::ir::IrGenError;
    /// use netsuke::localization::{self, keys};
    /// use serde::ser::Error as _;
    ///
    /// let err = IrGenError::InvalidCommand {
    ///     command: "echo $in".into(),
    ///     snippet: "echo $in".into(),
    ///     message: localization::message(keys::IR_INVALID_COMMAND).with_arg("snippet", "echo $in"),
    /// };
    /// assert!(matches!(err, IrGenError::InvalidCommand { command, .. } if command == "echo $in"));
    /// ```
    #[error("{message}")]
    InvalidCommand {
        /// Original command string provided in the manifest.
        command: String,
        /// Rendered snippet that failed validation.
        snippet: String,
        /// Localized error message.
        message: LocalizedMessage,
    },
}
