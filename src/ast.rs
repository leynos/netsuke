//! Netsuke manifest Abstract Syntax Tree structures.
//!
//! This module defines the data structures used to represent a parsed
//! `Netsukefile`. They mirror the YAML schema described in the design
//! document and are deserialised with `serde_yaml`.

use serde::Deserialize;
use std::collections::HashMap;

/// Top-level manifest structure.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetsukeManifest {
    pub netsuke_version: String,

    #[serde(default)]
    pub vars: HashMap<String, serde_yaml::Value>,

    #[serde(default)]
    pub rules: Vec<Rule>,

    #[serde(default)]
    pub actions: Vec<Target>,

    pub targets: Vec<Target>,

    #[serde(default)]
    pub defaults: Vec<String>,
}

/// A reusable command template.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub name: String,
    pub recipe: Recipe,
    pub description: Option<String>,
    pub deps: Option<String>,
}

/// Execution style for rules and targets.
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Recipe {
    #[serde(alias = "command")]
    Command { command: String },
    #[serde(alias = "script")]
    Script { script: String },
    #[serde(alias = "rule")]
    Rule { rule: StringOrList },
}

/// A single build target.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub name: StringOrList,
    pub recipe: Recipe,

    #[serde(default)]
    pub sources: StringOrList,

    #[serde(default)]
    pub deps: StringOrList,

    #[serde(default)]
    pub order_only_deps: StringOrList,

    #[serde(default)]
    pub vars: HashMap<String, String>,

    #[serde(default)]
    pub phony: bool,

    #[serde(default)]
    pub always: bool,
}

/// A helper for fields that accept either a string or list of strings.
#[derive(Debug, Deserialize, Default)]
#[serde(untagged)]
pub enum StringOrList {
    #[default]
    Empty,
    String(String),
    List(Vec<String>),
}
