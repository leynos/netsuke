//! Build graph data structures for the intermediate representation (IR).
//!
//! Defines the core IR types: [`BuildGraph`] (the complete static build
//! graph), [`BuildEdge`] (a single target with its `inputs`, `implicit_deps`,
//! and `order_only_deps`), and [`Action`] (a deduplicated build rule plus
//! recipe).  `implicit_deps` mirror Ninja's `|` syntax — they trigger
//! rebuilds but are not passed to `{{ ins }}`. Consumed by [`crate::ninja_gen`]
//! for Ninja file emission and by [`super::cycle`] for cycle detection.

use crate::localization::LocalizedMessage;
use camino::{Utf8Path, Utf8PathBuf};
use serde::Serialize;
#[cfg(not(kani))]
use std::collections::HashMap;
use thiserror::Error;

use crate::ast::Recipe;

#[cfg(kani)]
#[path = "graph_kani_map.rs"]
mod kani_map;

#[cfg(kani)]
pub use kani_map::{IrHashMap, IrVec};

/// Map used by the IR graph.
#[cfg(not(kani))]
pub type IrHashMap<K, V> = HashMap<K, V>;

/// Arena used to own canonical build edges.
#[cfg(kani)]
pub type EdgeArena<T> = IrVec<T>;

/// Arena used to own canonical build edges.
#[cfg(not(kani))]
pub type EdgeArena<T> = Vec<T>;

/// The complete, static build graph.
#[derive(Debug, Default, Clone)]
pub struct BuildGraph {
    /// All unique actions in the build keyed by a stable hash.
    pub actions: IrHashMap<String, Action>,
    /// Canonical build edges, each owned exactly once.
    pub edges: EdgeArena<BuildEdge>,
    /// Output aliases indexed to their canonical producing edge.
    pub targets: IrHashMap<Utf8PathBuf, EdgeId>,
    /// Targets built when no explicit target is requested.
    pub default_targets: Vec<Utf8PathBuf>,
}

/// Identifies one canonical [`BuildEdge`] in a [`BuildGraph`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EdgeId(usize);

impl BuildGraph {
    /// Store `edge` once and index each of its explicit outputs to it.
    ///
    /// Callers must reject duplicate outputs before insertion so that this
    /// operation never replaces an existing output alias.
    pub fn insert_edge(&mut self, edge: BuildEdge) -> EdgeId {
        let edge_id = EdgeId(self.edges.len());
        self.edges.push(edge);
        if let Some(stored_edge) = self.edges.get(edge_id.0) {
            for output in &stored_edge.explicit_outputs {
                self.targets.insert(output.clone(), edge_id);
            }
        }
        edge_id
    }

    /// Resolve `output` to its canonical stored key and producing edge.
    #[cfg(not(kani))]
    #[must_use]
    pub fn target_for_output(&self, output: &Utf8Path) -> Option<(&Utf8Path, &BuildEdge)> {
        self.targets
            .get_key_value(output)
            .and_then(|(stored_output, edge_id)| {
                self.edges
                    .get(edge_id.0)
                    .map(|edge| (stored_output.as_path(), edge))
            })
    }

    /// Resolve `output` through Kani's bounded path-key model.
    #[cfg(kani)]
    #[must_use]
    pub fn target_for_output(&self, output: &Utf8Path) -> Option<(&Utf8Path, &BuildEdge)> {
        self.targets
            .get_key_value_path(output)
            .and_then(|(stored_output, edge_id)| {
                self.edges
                    .get(edge_id.0)
                    .map(|edge| (stored_output.as_path(), edge))
            })
    }

    /// Iterate the graph's canonical build edges in arena insertion order.
    pub fn edges(&self) -> impl Iterator<Item = &BuildEdge> {
        self.edges.iter()
    }

    /// Iterate every explicit output alias in the output index.
    pub fn output_paths(&self) -> impl Iterator<Item = &Utf8PathBuf> {
        self.targets.keys()
    }
}

/// Dependency scheduling policy carried by the domain build graph.
///
/// This IR type deliberately has no serialization responsibility. Manifest
/// spelling and Serde behaviour belong to [`crate::ast::DependencyOrder`] and
/// are converted explicitly while lowering the manifest into the graph.
///
/// ```compile_fail
/// use netsuke::ir::DependencyOrder;
///
/// // Serialize the manifest AST policy, not its lowered domain counterpart.
/// serde_json::to_string(&DependencyOrder::Parallel)?;
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DependencyOrder {
    /// Dependencies may run in any order allowed by the build scheduler.
    #[default]
    Parallel,
    /// Dependencies run in declaration order, one after another.
    Serial,
}

/// A reusable command analogous to a Ninja rule.

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Action {
    /// Recipe invoked by Ninja when this action is referenced.
    pub recipe: Recipe,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional human-readable description used by Ninja's `description` flag.
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional depfile path provided to Ninja's `depfile` attribute.
    pub depfile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional dependency format (`deps`) such as `gcc`.
    pub deps_format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional Ninja pool to throttle concurrent execution.
    pub pool: Option<String>,
    /// Flag mirroring Ninja's `restat` behaviour for timestamp stability.
    pub restat: bool,
}

/// A single build statement connecting inputs to outputs.
#[derive(Debug, Clone, PartialEq)]
pub struct BuildEdge {
    /// Identifier of the [`Action`] used for this edge.
    pub action_id: String,
    /// Explicit inputs that trigger a rebuild when changed.
    pub inputs: Vec<Utf8PathBuf>,
    /// Implicit dependencies that trigger a rebuild without entering recipes.
    pub implicit_deps: Vec<Utf8PathBuf>,
    /// Ordering policy applied to `implicit_deps` when they come from a
    /// manifest `deps` list. Parallel is the default; serial dependencies are
    /// lowered into staged Ninja dyndep gates by the generator.
    pub dependency_order: DependencyOrder,
    /// Outputs explicitly generated by the command.
    pub explicit_outputs: Vec<Utf8PathBuf>,
    /// Outputs implicitly generated by the command (Ninja `|`).
    pub implicit_outputs: Vec<Utf8PathBuf>,
    /// Order-only dependencies that do not trigger rebuilds (Ninja `||`).
    pub order_only_deps: Vec<Utf8PathBuf>,
    /// Output does not correspond to a real file.
    pub phony: bool,
    /// Run the command on every invocation regardless of timestamps.
    pub always: bool,
}

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
