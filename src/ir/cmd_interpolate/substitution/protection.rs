//! Classify whether a shell region permits recipe-marker lowering.
//!
//! Each shell scanner computes this classification from its own quote,
//! comment, and escape state, and the traversal consults it per character.
//! Keeping the type in its own module holds the traversal root within the
//! repository's module-size cap.

/// Classify whether the active shell region permits recipe-marker lowering.
#[derive(Clone, Copy)]
pub(super) enum MarkerProtection {
    /// Reject a marker because the shell region cannot safely encode it.
    Protected,
    /// Lower a marker using the current shell quote context.
    Unprotected,
}

impl MarkerProtection {
    /// Classify a shell-context predicate without exposing a Boolean at call sites.
    pub(super) const fn from_protected_region(is_protected: bool) -> Self {
        if is_protected {
            Self::Protected
        } else {
            Self::Unprotected
        }
    }
}
