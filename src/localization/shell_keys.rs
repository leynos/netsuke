//! Fluent message identifiers used by the recipe-shell template filters.
//!
//! Split from `keys.rs`, which sits at Whitaker's 400-line module cap. The
//! table keeps the same shape, so the build-time localization audit reads it
//! the same way, and `keys` re-exports every constant, so callers still write
//! `keys::STDLIB_SHELL_JOIN_LIMIT`.

define_keys! {
    STDLIB_SHELL_ARGS_ERROR => "stdlib.shell.args_error",
    STDLIB_SHELL_UNQUOTABLE => "stdlib.shell.unquotable",
    STDLIB_SHELL_QUOTE_NOT_STRING => "stdlib.shell.quote.not_string",
    STDLIB_SHELL_QUOTE_CONTROL_CHARACTER => "stdlib.shell.quote.control_character",
    STDLIB_SHELL_DIALECT_INVALID => "stdlib.shell.dialect_invalid",
    STDLIB_SHELL_DIALECT_NOT_STRING => "stdlib.shell.dialect_not_string",
    STDLIB_SHELL_JOIN_NOT_SEQUENCE => "stdlib.shell.join.not_sequence",
    STDLIB_SHELL_JOIN_ITEM_NOT_STRING => "stdlib.shell.join.item_not_string",
    STDLIB_SHELL_JOIN_LIMIT => "stdlib.shell.join.limit",
    STDLIB_SHELL_POSITIONAL_OPTION => "stdlib.shell.positional_option",
}
