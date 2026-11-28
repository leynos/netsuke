//! The reservation proof must not be constructible outside the refusal.
//!
//! The proof's fields are private to the module that declares them, so a caller
//! in any ancestor, sibling, or descendant module that is not the refusal
//! cannot build one. This fixture mirrors the proof's shape and tries the
//! construction a caller with the wrong idea would try; the compiler must
//! reject the private fields.

mod policy {
    /// Mirrors `phony_state::policy::ReservedStateChecked`.
    pub struct ReservedStateChecked {
        /// Canonical workspace the paths were resolved against.
        canonical_workspace: String,
        /// Every path the refusal judged.
        checked: Vec<String>,
    }
}

fn main() {
    let _ = policy::ReservedStateChecked {
        canonical_workspace: String::new(),
        checked: Vec::new(),
    };
}
