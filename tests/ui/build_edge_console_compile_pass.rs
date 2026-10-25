//! Compile the public console scheduling flag as an external crate would.

use netsuke::ast::{
    CommandIsolation, DependencyOrder as AstDependencyOrder, Recipe, StringOrList, Target,
};
use netsuke::ir::{BuildEdge, DependencyOrder as IrDependencyOrder};
use std::collections::HashMap;

/// Construct a public build edge that opts into Ninja's console pool.
#[must_use]
pub fn console_edge() -> BuildEdge {
    BuildEdge {
        action_id: "lint".into(),
        inputs: Vec::new(),
        implicit_deps: Vec::new(),
        dependency_order: IrDependencyOrder::Parallel,
        explicit_outputs: vec!["lint".into()],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
        console: true,
    }
}

/// Construct a public target that opts into Ninja's console pool.
#[must_use]
pub fn console_target() -> Target {
    Target {
        name: "lint".into(),
        recipe: Recipe::Command {
            command: StringOrList::from("cargo clippy"),
            isolation: CommandIsolation::Shared,
        },
        sources: StringOrList::Empty,
        deps: StringOrList::Empty,
        dependency_order: AstDependencyOrder::Parallel,
        order_only_deps: StringOrList::Empty,
        vars: HashMap::default(),
        phony: false,
        always: false,
        console: true,
        conditional: false,
        description: None,
    }
}
