//! Error formatting helpers for stable, deterministic test assertions.

use std::error::Error;

/// Join an error and its sources (outermost → root cause) for stable
/// assertions.
///
/// Works with any error implementing [`std::error::Error`]. Types like
/// [`miette::Report`] can be passed via [`AsRef::as_ref`].
///
/// # Examples
///
/// ```ignore
/// let err = std::io::Error::new(std::io::ErrorKind::Other, "oops");
/// assert_eq!(display_error_chain(&err), "oops");
/// ```
pub fn display_error_chain(e: &(dyn Error + 'static)) -> String {
    // `std::error::Error::sources` is unstable; traverse via `source` instead.
    let mut current: Option<&(dyn Error + 'static)> = Some(e);
    std::iter::from_fn(|| {
        let err = current?;
        current = err.source();
        Some(err.to_string())
    })
    .collect::<Vec<_>>()
    .join(": ")
}
