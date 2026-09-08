//! Rules about declarations that are unnecessary, inert, or duplicated.

use crate::lint::registry::Registered;

pub mod declarations;
pub mod duplication;

/// Register this module's rules.
#[must_use]
pub fn rules() -> Vec<Registered> {
    let mut rules = declarations::rules();
    rules.extend(duplication::rules());
    rules
}
