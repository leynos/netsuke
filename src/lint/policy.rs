//! Resolve per-rule severity from the `--rule` selectors.
//!
//! Policy resolution is pure and order-sensitive: selectors apply left to
//! right over the registry defaults, so a category selector followed by a rule
//! selector narrows it. Nothing here reads the environment, the terminal, or
//! the clock, which is what makes two runs over the same manifest and
//! configuration report the same findings.

use std::collections::BTreeMap;

use super::registry;
use super::rule::Category;
use super::severity::{SEVERITY_VALUES, Severity, parse_policy_severity};

/// A `--rule` selector that could not be applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyError {
    /// The selector was not written as `NAME=SEVERITY`.
    Malformed {
        /// The selector as supplied.
        selector: String,
    },
    /// The selector named neither a rule nor a category.
    UnknownName {
        /// The unrecognized name.
        name: String,
    },
    /// The selector named an unrecognized severity.
    UnknownSeverity {
        /// The rule or category the selector named.
        name: String,
        /// The unrecognized severity.
        severity: String,
    },
}

impl PolicyError {
    /// Render the reader-facing message for this failure.
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::Malformed { selector } => {
                format!("lint selector `{selector}` is not written as NAME=SEVERITY")
            }
            Self::UnknownName { name } => {
                format!("lint selector names `{name}`, which is neither a rule nor a category")
            }
            Self::UnknownSeverity { name, severity } => format!(
                "lint selector `{name}={severity}` names an unknown severity; \
                 expected one of {SEVERITY_VALUES}"
            ),
        }
    }
}

/// The severity every registered rule reports at for one run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Policy {
    /// Resolved severity per rule name; `None` marks a disabled rule.
    resolved: BTreeMap<&'static str, Option<Severity>>,
}

impl Policy {
    /// Resolve the registry defaults with no selectors applied.
    #[must_use]
    pub fn defaults() -> Self {
        Self {
            resolved: registry::all_meta()
                .map(|meta| (meta.name, meta.default_severity.severity()))
                .collect(),
        }
    }

    /// Resolve the registry defaults, then apply `selectors` in order.
    ///
    /// # Errors
    ///
    /// Returns the first selector that is malformed or names an unknown rule,
    /// category, or severity. An unknown name is an error rather than a
    /// warning so a typo in continuous-integration configuration fails loudly
    /// instead of silently widening or narrowing the run.
    pub fn resolve<S: AsRef<str>>(selectors: &[S]) -> Result<Self, PolicyError> {
        let mut policy = Self::defaults();
        for selector in selectors {
            policy.apply(selector.as_ref())?;
        }
        Ok(policy)
    }

    /// Apply one `NAME=SEVERITY` selector.
    fn apply(&mut self, selector: &str) -> Result<(), PolicyError> {
        let (raw_name, raw_severity) =
            selector
                .split_once('=')
                .ok_or_else(|| PolicyError::Malformed {
                    selector: selector.to_owned(),
                })?;
        let name = raw_name.trim();
        let severity = parse_policy_severity(raw_severity.trim()).map_err(|value| {
            PolicyError::UnknownSeverity {
                name: name.to_owned(),
                severity: value.to_owned(),
            }
        })?;
        self.assign(name, severity)
    }

    /// Set `severity` on the rule or category named `name`.
    fn assign(&mut self, name: &str, severity: Option<Severity>) -> Result<(), PolicyError> {
        if let Some(entry) = registry::meta_by_name(name) {
            self.resolved.insert(entry.name, severity);
            return Ok(());
        }
        let category = Category::parse(name).ok_or_else(|| PolicyError::UnknownName {
            name: name.to_owned(),
        })?;
        for meta in registry::all_meta().filter(|meta| meta.category == category) {
            self.resolved.insert(meta.name, severity);
        }
        Ok(())
    }

    /// Report the severity `name` runs at, or `None` when it is disabled.
    #[must_use]
    pub fn severity_of(&self, name: &str) -> Option<Severity> {
        self.resolved.get(name).copied().flatten()
    }

    /// Report whether no rule is enabled.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.resolved.values().all(Option::is_none)
    }
}

#[cfg(test)]
#[path = "policy_tests.rs"]
mod tests;
