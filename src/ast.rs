//! Netsuke manifest Abstract Syntax Tree structures.
//!
//! This module defines the data structures used to represent a parsed
//! `Netsukefile`. They mirror the YAML schema described in the design
//! document and are deserialised with `serde_yml`.
//!
//! The following example shows how to parse a minimal manifest string:
//!
//! ```rust
//! use netsuke::ast::NetsukeManifest;
//! use netsuke::ast::StringOrList;
//!
//! let yaml = "netsuke_version: \"1.0.0\"\ntargets:\n  - name: hello\n    command: \"echo hi\"";
//! let manifest: NetsukeManifest = serde_yml::from_str(yaml).expect("parse");
//! if let StringOrList::String(name) = &manifest.targets[0].name {
//!     assert_eq!(name, "hello");
//! }
//! ```

use semver::Version;
use serde::{Deserialize, Serialize, de::Deserializer};
use std::collections::HashMap;

/// Map type for `vars` blocks, preserving YAML values.
pub type Vars = HashMap<String, serde_yml::Value>;

fn deserialize_actions<'de, D>(deserializer: D) -> Result<Vec<Target>, D::Error>
where
    D: Deserializer<'de>,
{
    let mut actions = Vec::<Target>::deserialize(deserializer)?;
    for action in &mut actions {
        action.phony = true;
    }
    Ok(actions)
}

/// Top-level manifest structure parsed from a `Netsukefile`.
///
/// Each field mirrors a key in the YAML manifest. Optional collections default
/// to empty to simplify deserialisation.
///
/// ```yaml
/// netsuke_version: "1.0.0"
/// actions: []
/// targets:
///   - name: hello
///     command: echo hi
/// ```
///
/// ```rust
/// use netsuke::ast::NetsukeManifest;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let yaml = "netsuke_version: \"1.0.0\"\ntargets:\n  - name: hello\n    command: echo hi";
/// let manifest: NetsukeManifest = serde_yml::from_str(yaml)?;
/// assert_eq!(manifest.targets.len(), 1);
/// # Ok(()) }
/// ```
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NetsukeManifest {
    /// Semantic version of the manifest format.
    pub netsuke_version: Version,

    /// Global key/value pairs available to recipes.
    #[serde(default)]
    pub vars: Vars,

    /// Named rule templates that can be referenced by targets.
    #[serde(default)]
    pub rules: Vec<Rule>,

    /// Optional setup actions executed before normal targets. Each action is
    /// implicitly marked as `phony` during deserialisation.
    #[serde(default, deserialize_with = "deserialize_actions")]
    pub actions: Vec<Target>,

    /// Primary build targets.
    pub targets: Vec<Target>,

    /// Names of targets built when no command line target is supplied.
    #[serde(default)]
    pub defaults: Vec<String>,
}

/// A reusable command template.
///
/// A rule encapsulates a snippet of work that can be referenced by multiple
/// targets. It may define a command line, a script block, or delegate to another
/// named rule. Dependencies may be specified as either a single string or a
/// list of strings.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    /// Unique identifier used by targets to reference this rule.
    pub name: String,
    /// The action executed when the rule is invoked.
    #[serde(flatten)]
    pub recipe: Recipe,
    /// Optional human-friendly summary.
    pub description: Option<String>,
    /// Prerequisites for the rule. Empty by default.
    #[serde(default)]
    pub deps: StringOrList,
}

/// Execution style for rules and targets.
///
/// Exactly one variant must be provided for a rule or target. The fields are
/// flattened in the manifest, so the presence of `command`, `script`, or `rule`
/// determines the variant.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Recipe {
    /// A single shell command.
    Command { command: String },
    /// An embedded multi-line script.
    Script { script: String },
    /// Invoke another named rule.
    Rule { rule: StringOrList },
}

impl<'de> Deserialize<'de> for Recipe {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawRecipe {
            command: Option<String>,
            script: Option<String>,
            rule: Option<StringOrList>,
        }

        let raw = RawRecipe::deserialize(deserializer)?;
        let present: Vec<&str> = [
            ("command", raw.command.is_some()),
            ("script", raw.script.is_some()),
            ("rule", raw.rule.is_some()),
        ]
        .into_iter()
        .filter_map(|(name, is_present)| is_present.then_some(name))
        .collect();

        match present.as_slice() {
            ["command"] => Ok(Self::Command {
                command: raw.command.expect("checked"),
            }),
            ["script"] => Ok(Self::Script {
                script: raw.script.expect("checked"),
            }),
            ["rule"] => Ok(Self::Rule {
                rule: raw.rule.expect("checked"),
            }),
            [] => Err(serde::de::Error::custom(
                "missing one of command, script, or rule",
            )),
            fields => Err(serde::de::Error::custom(format!(
                "fields {} are mutually exclusive",
                fields.join(", ")
            ))),
        }
    }
}

/// A single build target.
///
/// Targets describe the files produced by a rule and their dependencies.
/// `phony` targets are always considered out of date, while `always` targets are
/// regenerated even if their inputs are unchanged.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    /// Output file or files.
    pub name: StringOrList,
    /// How the target should be built.
    #[serde(flatten)]
    pub recipe: Recipe,

    /// Input files consumed by the recipe.
    #[serde(default)]
    pub sources: StringOrList,

    /// Normal prerequisites that must be built first.
    #[serde(default)]
    pub deps: StringOrList,

    /// Dependencies that do not cause a rebuild when changed.
    #[serde(default)]
    pub order_only_deps: StringOrList,

    /// Target-scoped variables available during command execution.
    #[serde(default)]
    pub vars: Vars,

    /// Declares that the target does not correspond to a real file.
    #[serde(default)]
    pub phony: bool,

    /// Force the recipe to run even if the outputs are up to date.
    #[serde(default)]
    pub always: bool,
}

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
