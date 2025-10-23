//! Netsuke manifest Abstract Syntax Tree structures.
//!
//! This module defines the data structures used to represent a parsed
//! `Netsukefile`. They mirror the YAML schema described in the design
//! document and are deserialised with `serde_saphyr`.
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

/// Map type for `vars` blocks, preserving JSON values produced by the YAML
/// parser.
pub type Vars = HashMap<String, serde_json::Value>;

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
    Command {
        /// Shell command executed verbatim by Ninja.
        command: String,
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRecipe {
    command: Option<String>,
    script: Option<String>,
    rule: Option<StringOrList>,
}

impl<'de> Deserialize<'de> for Recipe {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawRecipe::deserialize(deserializer)?;
        let RawRecipe {
            command: command_opt,
            script: script_opt,
            rule: rule_opt,
        } = raw;
        let present: Vec<&str> = [
            ("command", command_opt.is_some()),
            ("script", script_opt.is_some()),
            ("rule", rule_opt.is_some()),
        ]
        .into_iter()
        .filter_map(|(name, is_present)| is_present.then_some(name))
        .collect();

        match (command_opt, script_opt, rule_opt) {
            (Some(command), None, None) => Ok(Self::Command { command }),
            (None, Some(script), None) => Ok(Self::Script { script }),
            (None, None, Some(rule)) => Ok(Self::Rule { rule }),
            (None, None, None) => Err(serde::de::Error::custom(
                "missing one of command, script, or rule",
            )),
            _ => Err(serde::de::Error::custom(format!(
                "fields {} are mutually exclusive",
                present.join(", ")
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
