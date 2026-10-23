//! Closed JSON wire types for the `help targets --edges` graph projection.
//!
//! This module projects only the already-loaded manifest query result. It does
//! not render recipes or expose the manifest AST's serialization shape.

use std::collections::HashSet;

use serde::Serialize;

use crate::ast::{DependencyOrder, NetsukeManifest, Recipe, StringOrList, Target};

/// The stable v1 graph payload nested inside the versioned help result.
#[derive(Debug, Serialize)]
pub(super) struct GraphJson {
    /// Manifest defaults, in authored order and including duplicates.
    defaults: Vec<String>,
    /// Rule declarations in manifest order.
    rules: Vec<RuleJson>,
    /// Action declarations followed by target declarations.
    nodes: Vec<NodeJson>,
}

/// One rule declaration and its raw recipe body.
#[derive(Debug, Serialize)]
struct RuleJson {
    /// Rule selector used by target recipes.
    name: String,
    /// Rendered discovery description, or JSON null when absent.
    description: Option<String>,
    /// Recipe text retained by manifest-query mode.
    recipe: RecipeJson,
}

/// One declaration-level action or target node.
#[derive(Debug, Serialize)]
struct NodeJson {
    /// Whether the declaration belongs to the actions or targets section.
    kind: NodeKind,
    /// Expanded names and exact-membership default markers.
    names: Vec<NodeNameJson>,
    /// Rendered discovery description, or JSON null when absent.
    description: Option<String>,
    /// Whether query mode left this declaration's `when` condition unresolved.
    conditional: bool,
    /// Whether the node is treated as phony by the build graph.
    phony: bool,
    /// Whether the recipe is forced to run on every build.
    always: bool,
    /// Ordering applied to the `deps` edge list.
    dependency_order: DependencyOrderJson,
    /// Authored dependency edges, grouped by their manifest field.
    edges: EdgeJson,
    /// Recipe source, without rendering command or script bodies.
    recipe: RecipeJson,
}

/// A resolved name belonging to one declaration.
#[derive(Debug, Serialize)]
struct NodeNameJson {
    /// Resolved name emitted by manifest query expansion.
    name: String,
    /// Whether this exact name appears in the manifest defaults list.
    default: bool,
}

/// The three authored dependency edge classes.
#[derive(Debug, Serialize)]
struct EdgeJson {
    /// Input files consumed by the recipe.
    sources: Vec<String>,
    /// Normal prerequisites.
    deps: Vec<String>,
    /// Prerequisites that do not cause a rebuild when changed.
    order_only_deps: Vec<String>,
}

/// A recipe's declaration section.
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum NodeKind {
    /// An action declaration.
    Action,
    /// A build target declaration.
    Target,
}

/// The ordering policy for a target's normal dependency list.
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum DependencyOrderJson {
    /// Dependencies may run in any order allowed by the scheduler.
    Parallel,
    /// Dependencies run in declaration order.
    Serial,
}

/// A closed recipe object that is independent of the manifest input schema.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum RecipeJson {
    /// One raw shell command or an ordered list of raw shell commands.
    Command {
        /// Whether the manifest used a string or list command form.
        form: CommandForm,
        /// Raw command strings in execution order.
        commands: Vec<String>,
    },
    /// Raw script body.
    Script {
        /// Script text retained by manifest-query mode.
        script: String,
    },
    /// Rule selector names, rendered by the restricted query surface.
    Rule {
        /// Selectors in declaration order.
        names: Vec<String>,
    },
    /// Internal empty-command marker for dependency-only targets.
    DependencyOnly,
}

/// The source representation of a legacy command recipe.
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum CommandForm {
    /// The manifest supplied one command string.
    String,
    /// The manifest supplied an ordered command list.
    List,
}

impl GraphJson {
    /// Project an already-loaded query manifest into the v1 graph payload.
    pub(super) fn from_manifest(manifest: &NetsukeManifest) -> Self {
        let defaults: HashSet<&str> = manifest.defaults.iter().map(String::as_str).collect();
        let mut nodes = Vec::with_capacity(manifest.actions.len() + manifest.targets.len());
        nodes.extend(
            manifest
                .actions
                .iter()
                .map(|target| node_json(target, NodeKind::Action, &defaults)),
        );
        nodes.extend(
            manifest
                .targets
                .iter()
                .map(|target| node_json(target, NodeKind::Target, &defaults)),
        );

        Self {
            defaults: manifest.defaults.clone(),
            rules: manifest
                .rules
                .iter()
                .map(|rule| RuleJson {
                    name: rule.name.clone(),
                    description: rule.description.clone(),
                    recipe: recipe_json(&rule.recipe),
                })
                .collect(),
            nodes,
        }
    }
}

/// Project one target declaration without expanding its names into nodes.
fn node_json(target: &Target, kind: NodeKind, defaults: &HashSet<&str>) -> NodeJson {
    NodeJson {
        kind,
        names: target.name.map_each(|name| NodeNameJson {
            name: name.to_owned(),
            default: defaults.contains(name),
        }),
        description: target.description.clone(),
        conditional: target.conditional,
        phony: target.phony,
        always: target.always,
        dependency_order: match target.dependency_order {
            DependencyOrder::Parallel => DependencyOrderJson::Parallel,
            DependencyOrder::Serial => DependencyOrderJson::Serial,
        },
        edges: EdgeJson {
            sources: target.sources.to_string_vec(),
            deps: target.deps.to_string_vec(),
            order_only_deps: target.order_only_deps.to_string_vec(),
        },
        recipe: recipe_json(&target.recipe),
    }
}

/// Convert an AST recipe into the export's closed recipe vocabulary.
fn recipe_json(recipe: &Recipe) -> RecipeJson {
    match recipe {
        Recipe::Command {
            command: StringOrList::String(command),
            ..
        } => RecipeJson::Command {
            form: CommandForm::String,
            commands: vec![command.clone()],
        },
        Recipe::Command {
            command: StringOrList::List(commands),
            ..
        } => RecipeJson::Command {
            form: CommandForm::List,
            commands: commands.clone(),
        },
        Recipe::Command {
            command: StringOrList::Empty,
            ..
        } => RecipeJson::DependencyOnly,
        Recipe::Script { script } => RecipeJson::Script {
            script: script.clone(),
        },
        Recipe::Rule { rule } => RecipeJson::Rule {
            names: rule.to_string_vec(),
        },
    }
}
