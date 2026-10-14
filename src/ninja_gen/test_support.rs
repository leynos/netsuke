//! Shared test constructors for Ninja generation modules.

use crate::{
    ast::{CommandIsolation, Recipe, StringOrList},
    ir::Action,
};

/// Construct a command action with the stable default metadata used in tests.
pub(super) const fn command_action(command: StringOrList) -> Action {
    Action {
        recipe: Recipe::Command {
            command,
            isolation: CommandIsolation::Shared,
        },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    }
}
