//! Severity levels and the failure threshold for manifest lint findings.
//!
//! Severity is deliberately a small closed vocabulary shared by the rule
//! registry, the policy selectors, the human renderer, and the JSON schema, so
//! that a value written in a configuration file, printed in a diagnostic, and
//! parsed by an agent is always the same word.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// How loudly a lint finding is reported.
///
/// The names match the severity vocabulary already used by Netsuke's JSON
/// diagnostic schema, so a lint finding and a compiler diagnostic are directly
/// comparable by machine consumers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// A suggestion that never fails a default run.
    Advice,
    /// A likely defect that an author should look at.
    Warning,
    /// A defect that should stop a build pipeline.
    Error,
}

impl Severity {
    /// Every severity, ordered from least to most severe.
    pub const ALL: [Self; 3] = [Self::Advice, Self::Warning, Self::Error];

    /// Name this severity using the schema's lowercase spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Advice => "advice",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }

    /// Project this severity onto the `miette` severity it renders as.
    #[must_use]
    pub const fn to_miette(self) -> miette::Severity {
        match self {
            Self::Advice => miette::Severity::Advice,
            Self::Warning => miette::Severity::Warning,
            Self::Error => miette::Severity::Error,
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The severity a rule reports at unless a policy selector overrides it.
///
/// `Off` marks a rule that encodes a project convention rather than a defect.
/// Such a rule runs only when a selector names it, which keeps the default run
/// free of findings that depend on house style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultSeverity {
    /// The rule runs by default at this severity.
    On(Severity),
    /// The rule runs only when a policy selector enables it.
    Off,
}

impl DefaultSeverity {
    /// Report the configured severity, or `None` when the rule is opt-in.
    #[must_use]
    pub const fn severity(self) -> Option<Severity> {
        match self {
            Self::On(severity) => Some(severity),
            Self::Off => None,
        }
    }

    /// Name this default for `--explain` output and the rule reference.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::On(severity) => severity.as_str(),
            Self::Off => "off",
        }
    }
}

/// The threshold at or above which findings fail `netsuke check`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FailOn {
    /// Fail on any advisory or worse.
    Advice,
    /// Fail on any warning or worse.
    Warning,
    /// Fail on errors only.
    #[default]
    Error,
    /// Never fail because of a finding.
    Never,
}

impl FailOn {
    /// Report whether `severity` reaches this threshold.
    #[must_use]
    pub const fn is_reached_by(self, severity: Severity) -> bool {
        match self {
            Self::Never => false,
            Self::Advice => true,
            Self::Warning => matches!(severity, Severity::Warning | Severity::Error),
            Self::Error => matches!(severity, Severity::Error),
        }
    }

    /// Name this threshold using its command-line spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Advice => "advice",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Never => "never",
        }
    }
}

impl fmt::Display for FailOn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The accepted spellings of a severity, listed in diagnostics.
pub const SEVERITY_VALUES: &str = "off, advice, warning, error";

/// The accepted spellings of a failure threshold, listed in diagnostics.
pub const FAIL_ON_VALUES: &str = "advice, warning, error, never";

/// Parse a policy severity, where `off` disables the rule.
///
/// # Errors
///
/// Returns the unrecognized spelling so the caller can name it in a
/// diagnostic that also lists the accepted values.
pub fn parse_policy_severity(text: &str) -> Result<Option<Severity>, &str> {
    match text {
        "off" => Ok(None),
        "advice" => Ok(Some(Severity::Advice)),
        "warning" => Ok(Some(Severity::Warning)),
        "error" => Ok(Some(Severity::Error)),
        other => Err(other),
    }
}

impl FromStr for FailOn {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
            "advice" => Ok(Self::Advice),
            "warning" => Ok(Self::Warning),
            "error" => Ok(Self::Error),
            "never" => Ok(Self::Never),
            other => Err(other.to_owned()),
        }
    }
}

#[cfg(test)]
#[path = "severity_tests.rs"]
mod tests;
