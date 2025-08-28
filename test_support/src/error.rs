use miette::Report;

/// Join an error and its sources (outermost → root cause) for stable
/// assertions.
///
/// ```ignore
/// use miette::Report;
/// let err = Report::msg("oops");
/// assert_eq!(display_error_chain(&err), "oops");
/// ```
pub fn display_error_chain(e: &Report) -> String {
    let mut out = String::new();
    for (i, cause) in e.chain().enumerate() {
        if i > 0 {
            out.push_str(": ");
        }
        out.push_str(&cause.to_string());
    }
    out
}
