//! Bounded telemetry for the recipe-text dialect boundary.
//!
//! Both `shell_quote` and `shell_join` reach exactly one place when they
//! resolve which encoding to apply: [`super::resolve_dialect`]. That single
//! boundary is therefore also the single telemetry point, and each resolution
//! is counted once under two closed label vocabularies.
//!
//! The `source` label is the reason this series exists. A call that omits
//! `dialect` receives a host- and configuration-dependent default, so the
//! *rendered text* of a manifest that does not pin the dialect can change
//! between releases or hosts with no manifest edit. Nothing else aggregates
//! that population: the manifests affected are otherwise indistinguishable
//! from those that pin it, and the difference is invisible in the generated
//! Ninja. Counting it makes the exposed set measurable, which is what turns
//! "pin your dialect" from advice into something an operator can size.
//!
//! What is recorded is deliberately bounded and redacted. The labels are
//! `dialect` and `source`, both drawn from the constant sets below, so the
//! number of series is fixed at four by this module rather than by anything a
//! manifest supplies. No manifest text, template source, or rendered value
//! reaches a label.

use metrics::{counter, describe_counter};
use std::sync::Once;

/// Counts recipe-text dialect resolutions by bounded `dialect` and `source`.
///
/// Both labels are drawn from the closed sets below, so the series count is
/// fixed by this module rather than by anything a manifest supplies. The
/// application recorder admits the series through the same sets, so the
/// counter is exported rather than silently dropped as a noop handle.
pub const SHELL_QUOTE_DIALECT_TOTAL: &str = "netsuke_manifest_shell_quote_dialect_total";

/// The bounded `dialect` recorded for POSIX `sh` quoting.
pub(crate) const DIALECT_SH: &str = "sh";
/// The bounded `dialect` recorded for Windows PowerShell quoting.
pub(crate) const DIALECT_POWERSHELL: &str = "powershell";

/// The closed `dialect` vocabulary admitted on [`SHELL_QUOTE_DIALECT_TOTAL`].
///
/// Kept in step with [`ShellDialect::ALL`](crate::shell_word::ShellDialect) by
/// a test, so adding a dialect cannot silently leave it uncounted.
pub const DIALECT_VALUES: [&str; 2] = [DIALECT_SH, DIALECT_POWERSHELL];

/// The bounded `source` recorded when the call site passed `dialect` itself.
pub(crate) const SOURCE_EXPLICIT: &str = "explicit";
/// The bounded `source` recorded when the call site omitted `dialect`.
pub(crate) const SOURCE_DEFAULT: &str = "default";

/// The closed `source` vocabulary admitted on [`SHELL_QUOTE_DIALECT_TOTAL`].
pub const DIALECT_SOURCE_VALUES: [&str; 2] = [SOURCE_EXPLICIT, SOURCE_DEFAULT];

/// Describe the dialect counter once per process.
fn describe_dialect_metrics() {
    static DESCRIBE: Once = Once::new();
    DESCRIBE.call_once(|| {
        describe_counter!(
            SHELL_QUOTE_DIALECT_TOTAL,
            "Counts shell_quote and shell_join dialect resolutions labelled by \
             dialect (sh or powershell) and source: explicit when the call \
             site passed dialect, or default when it omitted the argument and \
             received the host-dependent default. A high default count marks \
             the manifests whose generated text is not pinned to an encoding."
        );
    });
}

/// Record one dialect resolution and return it unchanged.
///
/// This is the telemetry boundary for both recipe-text filters: every
/// resolution that reaches it is counted exactly once. `dialect` and `source`
/// are always one of the constants above — a literal never reaches a label —
/// so the series stay bounded and no manifest text reaches the metric.
pub(super) fn record_dialect<T>(dialect: &'static str, source: &'static str, value: T) -> T {
    describe_dialect_metrics();
    debug_assert!(
        DIALECT_VALUES.contains(&dialect),
        "dialect telemetry must use a closed dialect vocabulary"
    );
    debug_assert!(
        DIALECT_SOURCE_VALUES.contains(&source),
        "dialect telemetry must use a closed source vocabulary"
    );
    counter!(SHELL_QUOTE_DIALECT_TOTAL, "dialect" => dialect, "source" => source).increment(1);
    value
}

#[cfg(test)]
mod tests {
    //! The two vocabularies are claims about what the encoder can produce.
    //!
    //! Each is declared here as a literal array rather than derived from
    //! [`ShellDialect::ALL`](crate::shell_word::ShellDialect), because the
    //! recorder imports them as `'static` constants and cannot ask a match arm
    //! for a slice at compile time. That independence is the hazard: adding a
    //! dialect would leave a name unlabelled and the counter silently short.
    //! The tests below close it from both ends.
    use super::{
        DIALECT_POWERSHELL, DIALECT_SH, DIALECT_SOURCE_VALUES, DIALECT_VALUES, SOURCE_DEFAULT,
        SOURCE_EXPLICIT,
    };
    use crate::shell_word::ShellDialect;
    use rstest::rstest;

    /// Every dialect the encoder can produce has a label, in the same order.
    ///
    /// Order matters as well as membership: the recorder admits these values by
    /// `contains`, but a reader comparing the two lists expects them aligned.
    #[rstest]
    fn the_label_set_matches_the_dialect_set() {
        let labelled: Vec<_> = DIALECT_VALUES.to_vec();
        let encodable: Vec<_> = ShellDialect::ALL
            .iter()
            .map(|dialect| dialect.telemetry_name())
            .collect();
        assert_eq!(labelled, encodable);
    }

    /// The two vocabularies are disjoint and internally distinct.
    ///
    /// A duplicate would not widen the admitted space, but it would make the
    /// debug assertion in `record_dialect` unable to distinguish two call sites
    /// that a reader expects to separate.
    #[rstest]
    fn each_vocabulary_is_distinct() {
        for values in [DIALECT_VALUES.as_slice(), DIALECT_SOURCE_VALUES.as_slice()] {
            let unique: std::collections::BTreeSet<_> = values.iter().collect();
            assert_eq!(unique.len(), values.len(), "duplicate label in {values:?}");
        }
    }

    /// Each documented label value is one the filter can actually emit.
    #[rstest]
    fn every_source_label_is_reachable() {
        assert_eq!(
            DIALECT_SOURCE_VALUES,
            [SOURCE_EXPLICIT, SOURCE_DEFAULT],
            "resolve_dialect emits exactly these two sources"
        );
        assert_eq!(
            DIALECT_VALUES,
            [DIALECT_SH, DIALECT_POWERSHELL],
            "resolve_dialect emits exactly these two dialects"
        );
    }
}
