//! Type-check the public command-list AST surface.

use netsuke::ast::{CommandIsolation, Recipe, StringOrList};

/// Return the command a command recipe carries.
#[must_use]
pub fn command(recipe: Recipe) -> Option<StringOrList> {
    match recipe {
        Recipe::Command { command, .. } => Some(command),
        Recipe::Script { .. } | Recipe::Rule { .. } => None,
    }
}

/// Return the recipe kind, matching every public variant.
#[must_use]
pub const fn recipe_kind(recipe: &Recipe) -> &'static str {
    match recipe {
        Recipe::Command { .. } => "command",
        Recipe::Script { .. } => "script",
        Recipe::Rule { .. } => "rule",
    }
}

/// Build each public `StringOrList` spelling an embedder constructs.
#[must_use]
pub fn string_or_list_spellings() -> [StringOrList; 3] {
    [
        StringOrList::from("borrowed"),
        StringOrList::from(String::from("owned")),
        StringOrList::from(vec![String::from("first"), String::from("second")]),
    ]
}

/// Construct a command recipe through its public fields.
#[must_use]
pub fn shared_command_recipe() -> Recipe {
    Recipe::Command {
        command: StringOrList::from("recipe"),
        isolation: CommandIsolation::Shared,
    }
}
