//! Control fixture for the reservation-proof compile tests.
//!
//! It mirrors the same modules as the failing fixtures and compiles cleanly,
//! so a harness or setup failure is distinguishable from the intended
//! rejection: if this fixture stops compiling, the failing fixtures prove
//! nothing.

mod policy {
    /// Mirrors `phony_state::policy::ReservedStateChecked`.
    pub struct ReservedStateChecked {
        /// Canonical workspace the paths were resolved against.
        canonical_workspace: String,
        /// Every path the refusal judged.
        checked: Vec<String>,
    }
}

mod reset {
    use super::policy::ReservedStateChecked;

    /// Stand-in for the workspace handle the real reset takes.
    pub struct Workspace;

    /// Mirror of `phony_state::reset::reset_force_node`.
    pub fn reset_force_node(_workspace: &Workspace, _checked: &ReservedStateChecked) {}
}

fn main() {
    let _ = reset::Workspace;
}
