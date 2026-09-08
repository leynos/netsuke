//! Rules where a canonical alternative reads better or is easier to discover.

use crate::lint::registry::Registered;

pub mod descriptions;
pub mod recipe_shape;

/// Register this module's rules.
#[must_use]
pub fn rules() -> Vec<Registered> {
    let mut rules = recipe_shape::rules();
    rules.extend(descriptions::rules());
    rules
}
