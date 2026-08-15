//! Shared test constructors for Ninja generation modules.

use crate::{
    ast::{Recipe, StringOrList},
    ir::Action,
};

/// Construct a command action with the stable default metadata used in tests.
pub(super) const fn command_action(command: StringOrList) -> Action {
    Action {
        recipe: Recipe::Command { command },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    }
}
