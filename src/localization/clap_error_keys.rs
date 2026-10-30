//! Fluent message identifiers for the clap errors Netsuke localizes.
//!
//! Upstream `ortho_l10n` ships only part of clap's stable error matrix, so
//! Netsuke declares the remaining identifiers itself. The table lives apart
//! from `keys.rs`, which sits at Whitaker's 400-line module cap, and keeps the
//! same shape so the build-time localization audit reads it the same way;
//! `keys` re-exports every constant, so callers still write
//! `keys::CLAP_ERROR_INVALID_VALUE`.

define_keys! {
    CLAP_ERROR_MISSING_ARGUMENT => "clap-error-missing-argument",
    CLAP_ERROR_MISSING_SUBCOMMAND => "clap-error-missing-subcommand",
    CLAP_ERROR_UNKNOWN_ARGUMENT => "clap-error-unknown-argument",
    CLAP_ERROR_INVALID_VALUE => "clap-error-invalid-value",
    CLAP_ERROR_TOO_MANY_VALUES => "clap-error-too-many-values",
    CLAP_ERROR_INVALID_SUBCOMMAND => "clap-error-invalid-subcommand",
    CLAP_ERROR_VALUE_VALIDATION => "clap-error-value-validation",
    // Derived from `ErrorKind::InvalidUtf8`; keep "utf8" unhyphenated. A `///`
    // doc comment here would become an attribute token `define_keys!` rejects.
    CLAP_ERROR_INVALID_UTF8 => "clap-error-invalid-utf8",
}
