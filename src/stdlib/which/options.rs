//! Parse and hold options for the `which` filter and function.

use minijinja::value::Kwargs;

use crate::localization::{self, keys};

use super::resolve_error::ResolveError;

/// Define the executable-search domain beyond directories explicitly in `PATH`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub(crate) enum CwdMode {
    #[default]
    /// Search directories named by `PATH`, including its empty current-directory entries.
    Auto,
    /// Search the current directory before directories named by `PATH`.
    Always,
    /// Search only non-empty directories explicitly named by `PATH`.
    Never,
    /// Search `PATH` and then recursively discover executables below the workspace root.
    WorkspaceRecursive,
}

impl CwdMode {
    /// Parse a mode name, returning `None` for unknown values.
    pub(super) fn parse(value: &str) -> Option<Self> {
        parse_cwd_mode(value)
    }
}

/// Map a mode name to its `CwdMode`.
fn parse_cwd_mode(value: &str) -> Option<CwdMode> {
    match value {
        "auto" => Some(CwdMode::Auto),
        "always" => Some(CwdMode::Always),
        "never" => Some(CwdMode::Never),
        "workspace-recursive" => Some(CwdMode::WorkspaceRecursive),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for `cwd_mode` parsing.

    use super::CwdMode;

    #[test]
    fn parses_workspace_recursive_mode() {
        assert_eq!(
            CwdMode::parse("workspace-recursive"),
            Some(CwdMode::WorkspaceRecursive)
        );
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
/// Options controlling a single `which` resolution.
pub(crate) struct WhichOptions {
    /// Return every executable match instead of stopping at the first.
    pub(super) all: bool,
    /// Canonicalize matched paths before returning them.
    pub(super) canonical: bool,
    /// Bypass the resolver cache for this resolution.
    pub(super) fresh: bool,
    /// How the current working directory is searched.
    pub(super) cwd_mode: CwdMode,
}

impl WhichOptions {
    /// Read the option values from template keyword arguments.
    ///
    /// # Errors
    ///
    /// Returns a [`ResolveError`] when a keyword cannot be extracted or the
    /// `cwd_mode` value names an invalid mode.
    pub(crate) fn from_kwargs(kwargs: &Kwargs) -> Result<Self, ResolveError> {
        let all = kwargs.get::<Option<bool>>("all")?.unwrap_or(false);
        let canonical = kwargs.get::<Option<bool>>("canonical")?.unwrap_or(false);
        let fresh = kwargs.get::<Option<bool>>("fresh")?.unwrap_or(false);
        let cwd_mode = kwargs
            .get::<Option<String>>("cwd_mode")?
            .map(|mode| {
                let lower = mode.to_ascii_lowercase();
                CwdMode::parse(&lower).ok_or_else(|| {
                    ResolveError::args(
                        localization::message(keys::STDLIB_WHICH_CWD_MODE_INVALID)
                            .with_arg("mode", mode),
                    )
                })
            })
            .transpose()?;
        Ok(Self {
            all,
            canonical,
            fresh,
            cwd_mode: cwd_mode.unwrap_or_default(),
        })
    }

    /// Clone the options with cache-irrelevant fields cleared, for cache keys.
    pub(crate) fn cache_key_view(&self) -> Self {
        let mut clone = self.clone();
        clone.fresh = false;
        clone
    }
}

impl From<minijinja::Error> for ResolveError {
    fn from(value: minijinja::Error) -> Self {
        Self::args(value)
    }
}
