//! Parse and hold options for the `which` filter and function.

use minijinja::value::Kwargs;

use crate::localization::{self, keys};

use super::resolve_error::ResolveError;

/// How the current working directory contributes to the PATH search.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub(crate) enum CwdMode {
    #[default]
    /// Search the current directory only when it appears in `PATH`.
    Auto,
    /// Always search the current directory, ahead of the PATH entries.
    Always,
    /// Never search the current directory.
    Never,
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
        _ => None,
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
/// Options controlling a single `which` resolution.
pub(crate) struct WhichOptions {
    /// Return every executable match instead of stopping at the first.
    pub(super) all: bool,
    /// Canonicalise matched paths before returning them.
    pub(super) canonical: bool,
    /// Bypass the resolver cache for this resolution.
    pub(super) fresh: bool,
    /// How the current working directory is searched.
    pub(super) cwd_mode: CwdMode,
}

impl WhichOptions {
    /// Read the option values from template keyword arguments.
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
