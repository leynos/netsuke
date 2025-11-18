use minijinja::{Error, value::Kwargs};

use super::error::args_error;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum CwdMode {
    Auto,
    Always,
    Never,
}

impl CwdMode {
    pub(super) fn parse(value: &str) -> Option<Self> {
        match value {
            "auto" => Some(Self::Auto),
            "always" => Some(Self::Always),
            "never" => Some(Self::Never),
            _ => None,
        }
    }
}

impl Default for CwdMode {
    fn default() -> Self {
        Self::Auto
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct WhichOptions {
    pub(super) all: bool,
    pub(super) canonical: bool,
    pub(super) fresh: bool,
    pub(super) cwd_mode: CwdMode,
}

impl WhichOptions {
    pub(crate) fn from_kwargs(kwargs: &Kwargs) -> Result<Self, Error> {
        let all = kwargs.get::<Option<bool>>("all")?.unwrap_or(false);
        let canonical = kwargs.get::<Option<bool>>("canonical")?.unwrap_or(false);
        let fresh = kwargs.get::<Option<bool>>("fresh")?.unwrap_or(false);
        let cwd_mode = kwargs
            .get::<Option<String>>("cwd_mode")?
            .map(|mode| {
                let lower = mode.to_ascii_lowercase();
                CwdMode::parse(&lower).ok_or_else(|| {
                    args_error(format!(
                        "cwd_mode must be 'auto', 'always', or 'never', got '{mode}'",
                    ))
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
}
