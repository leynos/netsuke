//! Compile the public console scheduling flag as an external crate would.

use netsuke::ir::{BuildEdge, DependencyOrder};

/// Construct a public build edge that opts into Ninja's console pool.
#[must_use]
pub fn console_edge() -> BuildEdge {
    BuildEdge {
        action_id: "lint".into(),
        inputs: Vec::new(),
        implicit_deps: Vec::new(),
        dependency_order: DependencyOrder::Parallel,
        explicit_outputs: vec!["lint".into()],
        implicit_outputs: Vec::new(),
        order_only_deps: Vec::new(),
        phony: false,
        always: false,
        console: true,
    }
}
