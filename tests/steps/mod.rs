mod cli_steps;
mod ir_steps;
mod manifest_steps;
mod ninja_steps;
mod process_steps;

use miette::Report;

/// Join an error and its sources for stable assertions.
///
/// ```ignore
/// use miette::Report;
/// let err = Report::msg("oops");
/// assert_eq!(display_error_chain(&err), "oops");
/// ```
pub(crate) fn display_error_chain(e: &Report) -> String {
    e.chain()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(": ")
}
