//! Netsuke manifest Abstract Syntax Tree structures.
//!
//! This module defines the data structures used to represent a parsed
//! `Netsukefile`. They mirror the YAML schema described in the design
//! document and are deserialized with `serde_saphyr`.
//!
//! The following example shows how to parse a minimal manifest string:
//!
//! ```rust
//! use netsuke::ast::NetsukeManifest;
//! use netsuke::ast::StringOrList;
//!
//! let yaml = "netsuke_version: \"1.0.0\"\ntargets:\n  - name: hello\n    command: \"echo hi\"";
//! let manifest: NetsukeManifest = serde_saphyr::from_str(yaml).expect("parse");
//! if let StringOrList::String(name) = &manifest.targets[0].name {
//!     assert_eq!(name, "hello");
//! }
//! ```
//!
//! For most applications you should prefer the high-level
//! [`manifest::from_str`](crate::manifest::from_str) helper, which validates and
//! reports diagnostics consistently:
//!
//! ```rust
//! use netsuke::manifest;
//!
//! let yaml = "netsuke_version: \"1.0.0\"\ntargets:\n  - name: hello\n    command: \"echo hi\"";
//! let manifest = manifest::from_str(yaml).expect("parse");
//! assert_eq!(manifest.targets.len(), 1);
//! ```

use semver::Version;
use serde::{Deserialize, Serialize, de::Deserializer};
use std::collections::HashMap;
#[cfg(kani)]
use std::{collections::hash_map::DefaultHasher, hash::BuildHasherDefault};

mod dependency_order;
mod string_or_list;
mod target;

pub use dependency_order::DependencyOrder;
pub use string_or_list::StringOrList;
pub use target::Target;

/// Map type for `vars` blocks, preserving JSON values produced by the YAML
/// parser.
#[cfg(not(kani))]
pub type Vars = HashMap<String, serde_json::Value>;

/// Map type for `vars` blocks under Kani.
#[cfg(kani)]
pub type Vars = HashMap<String, serde_json::Value, BuildHasherDefault<DefaultHasher>>;
/// Stable schema error that the manifest adapter translates for its users.
pub(crate) const EMPTY_COMMAND_LIST_ERROR: &str = "command list must not be empty";

/// Deserialize an action sequence, marking each target as `phony`.
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

/// Definition of a reusable manifest macro registered with `MiniJinja`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MacroDefinition {
    /// Full macro signature as accepted by `MiniJinja`.
    pub signature: String,
    /// Body of the macro written using YAML block style.
    pub body: String,
}

/// Top-level manifest structure parsed from a `Netsukefile`.
///
/// Each field mirrors a key in the YAML manifest. Optional collections default
/// to empty to simplify deserialization.
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
/// let manifest: NetsukeManifest = serde_saphyr::from_str(yaml)?;
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

    /// Optional list of user-defined Jinja macros registered before rendering.
    #[serde(default)]
    pub macros: Vec<MacroDefinition>,

    /// Named rule templates that can be referenced by targets.
    #[serde(default)]
    pub rules: Vec<Rule>,

    /// Optional setup actions executed before normal targets. Each action is
    /// implicitly marked as `phony` during deserialization.
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
/// named rule.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    /// Unique identifier used by targets to reference this rule.
    pub name: String,
    /// The action executed when the rule is invoked.
    #[serde(flatten)]
    pub recipe: Recipe,
    /// Optional human-friendly summary.
    pub description: Option<String>,
}

/// Execution style for rules and targets.
///
/// Exactly one variant must be provided for a rule or target. The fields are
/// flattened in the manifest, so the presence of `command`, `script`, or `rule`
/// determines the variant.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Recipe {
    /// A shell command, given as a scalar or an ordered list executed by a
    /// fail-fast shell chain.
    Command {
        /// A scalar command passes through unchanged; list entries are
        /// evaluated in brace groups joined by a fail-fast `&&` chain.
        command: StringOrList,
    },
    /// An embedded multi-line script.
    Script {
        /// Shell script content rendered into a `printf %b` pipeline.
        script: String,
    },
    /// Invoke another named rule.
    Rule {
        /// Name or names of rules to execute.
        rule: StringOrList,
    },
}

/// Flattened recipe fields before deserialization selects a variant.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRecipe {
    /// Optional command line or ordered command list.
    command: Option<StringOrList>,
    /// Optional embedded script.
    script: Option<String>,
    /// Optional referenced rule name or names.
    rule: Option<StringOrList>,
}

impl<'de> Deserialize<'de> for Recipe {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawRecipe::deserialize(deserializer)?;
        let RawRecipe {
            command: command_field,
            script: script_field,
            rule: rule_field,
        } = raw;
        match (command_field, script_field, rule_field) {
            (Some(command), None, None) => match command {
                empty if empty.is_empty_content() => {
                    Err(serde::de::Error::custom(EMPTY_COMMAND_LIST_ERROR))
                }
                command_value => Ok(Self::Command {
                    command: command_value,
                }),
            },
            (None, Some(script), None) => Ok(Self::Script { script }),
            (None, None, Some(rule)) => Ok(Self::Rule { rule }),
            (None, None, None) => Err(serde::de::Error::custom(
                "missing one of command, script, or rule",
            )),
            (command_opt, script_opt, rule_opt) => {
                let present: Vec<&str> = [
                    ("command", command_opt.is_some()),
                    ("script", script_opt.is_some()),
                    ("rule", rule_opt.is_some()),
                ]
                .into_iter()
                .filter_map(|(name, is_present)| is_present.then_some(name))
                .collect();

                Err(serde::de::Error::custom(format!(
                    "fields {} are mutually exclusive",
                    present.join(", ")
                )))
            }
        }
    }
}
