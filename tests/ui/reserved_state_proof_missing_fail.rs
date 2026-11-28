//! The force-node reset must not compile without a reservation proof.
//!
//! `reset_force_node` requires the proof value the refusal returns, so a caller
//! that skipped the refusal has nothing to pass. This fixture mirrors the
//! signature and calls it without the proof, which the compiler must reject.

mod reset {
    /// Stand-in for the workspace handle the real reset takes.
    pub struct Workspace;

    /// Stand-in for the proof the refusal returns.
    pub struct ReservedStateChecked;

    /// Mirror of `phony_state::reset::reset_force_node`.
    pub fn reset_force_node(_workspace: &Workspace, _checked: &ReservedStateChecked) {}
}

fn main() {
    let workspace = reset::Workspace;
    reset::reset_force_node(&workspace);
}
