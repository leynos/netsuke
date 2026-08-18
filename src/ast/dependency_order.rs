//! Manifest syntax for ordering a target or action dependency list.

use serde::{Deserialize, Serialize};

/// Ordering policy applied to a target or action deps list.
///
/// Omission means [`DependencyOrder::Parallel`], preserving the existing
/// unordered-graph behaviour. A serial list starts each dependency only after
/// the preceding dependency has completed successfully.
///
/// ```yaml
/// targets:
///   - name: all
///     dependency_order: serial
///     deps:
///       - check-fmt
///       - test
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DependencyOrder {
    /// Dependencies may run in any order, subject to the Ninja scheduler.
    #[default]
    Parallel,
    /// Dependencies run in declaration order, one after another.
    Serial,
}
