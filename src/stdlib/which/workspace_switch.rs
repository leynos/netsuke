//! The `NETSUKE_WHICH_WORKSPACE` opt-out switch for the workspace fallback.
//!
//! A leaf module by design: `env::EnvSnapshot::capture` reads the variable at
//! the resolver's ambient boundary, translates the reading into
//! [`WorkspaceSwitch`], and stores that as snapshot data; `lookup::workspace`
//! asks the stored state whether to search. Placing the name and the domain
//! state here keeps both consumers pointing downward — the earlier
//! arrangement had `env` calling back into `lookup::workspace`, a module
//! cycle.
//!
//! The state is deliberately infrastructure-free: it names no
//! `std::env::VarError` and emits no diagnostics. Translating the platform
//! reading and warning about a mis-encoded value both belong to the adapter
//! that performs the read, so `env` owns the `From` conversion and the
//! non-UTF-8 warning.

/// The variable a user sets to switch the workspace fallback off.
pub(super) const WORKSPACE_FALLBACK_ENV: &str = "NETSUKE_WHICH_WORKSPACE";

/// The workspace switch as captured, in domain terms.
///
/// Three states rather than a bare `bool` because the cache fingerprint must
/// distinguish them — two resolutions differing only in this switch must not
/// share a cache entry — and because the adapter reports the non-UTF-8 case
/// to the user. `Hash` is derived, so the fingerprint hashes the state
/// directly instead of flattening a non-`Hash` platform error.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) enum WorkspaceSwitch {
    /// The variable was set to this value.
    Value(String),
    /// The variable was not set at all.
    Absent,
    /// The variable was set to a value that is not valid UTF-8.
    NotUnicode,
}

impl WorkspaceSwitch {
    /// Whether this state leaves the workspace fallback enabled.
    ///
    /// The switch is opt-out: an absent variable leaves the search on. A
    /// mis-encoded value switches it off rather than being treated as absent,
    /// because the user plainly meant to set something and guessing at the
    /// intent would be worse than declining to search.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // A disabling value switches the fallback off, case-insensitively.
    /// assert!(!WorkspaceSwitch::Value("OFF".to_owned()).enabled());
    /// // The switch is opt-out, so an unset variable leaves it enabled.
    /// assert!(WorkspaceSwitch::Absent.enabled());
    /// ```
    pub(super) fn enabled(&self) -> bool {
        match self {
            Self::Value(value) => {
                let normalised = value.to_ascii_lowercase();
                !matches!(normalised.as_str(), "0" | "false" | "off")
            }
            Self::Absent => true,
            Self::NotUnicode => false,
        }
    }
}

#[cfg(test)]
#[path = "lookup/workspace/fallback_tests.rs"]
mod fallback_tests;
