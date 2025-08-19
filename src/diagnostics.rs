use miette::{Context, IntoDiagnostic, Result};
use std::fmt::Display;

pub(crate) trait ResultExt<T> {
    fn diag(self, context: impl Display + Send + Sync + 'static) -> Result<T>;
    fn diag_with(self, f: impl FnOnce() -> String) -> Result<T>;
}

impl<T, E> ResultExt<T> for std::result::Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn diag(self, context: impl Display + Send + Sync + 'static) -> Result<T> {
        self.into_diagnostic().wrap_err(context)
    }

    fn diag_with(self, f: impl FnOnce() -> String) -> Result<T> {
        self.into_diagnostic().wrap_err_with(f)
    }
}
