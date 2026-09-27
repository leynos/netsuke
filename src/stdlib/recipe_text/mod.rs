//! Pure recipe-text filters: `shell_quote` and `shell_join`.
//!
//! Both encode template values as shell words for a named dialect, so a recipe
//! can carry an argument containing spaces, quotes, or metacharacters without
//! the author hand-rolling escaping. They are *pure* given a dialect: the only
//! host-dependent input is the default dialect, which is resolved once at
//! registration and disclosed by D6 rather than read per call.
//!
//! The encoding itself lives in [`crate::shell_word`], which is also what IR
//! lowering and Ninja rendering use. This module is the template-facing
//! adapter: it validates arguments, resolves the dialect, and hands the word to
//! that one implementation, so constraint 4's "exactly one implementation" has
//! a single call site to guard.

use minijinja::{
    Environment, Error, ErrorKind,
    value::{Kwargs, Rest, Value, ValueKind},
};

use crate::localization::{self, keys};
use crate::shell_word::{self, ShellDialect};

mod dialect_telemetry;

pub use dialect_telemetry::{DIALECT_SOURCE_VALUES, DIALECT_VALUES, SHELL_QUOTE_DIALECT_TOTAL};

/// Register the pure recipe-text filters on an environment.
///
/// `default` is the dialect used when a call omits its `dialect` keyword
/// argument. It is resolved once by the caller, not per call, so a render
/// cannot observe two different defaults.
pub(crate) fn register_filters(env: &mut Environment<'_>, default: ShellDialect) {
    env.add_filter(
        "shell_quote",
        move |value: Value, rest: Rest<Value>, kwargs: Kwargs| {
            quote_for_recipe(default, &value, &rest, &kwargs)
        },
    );
    env.add_filter(
        "shell_join",
        move |value: Value, rest: Rest<Value>, kwargs: Kwargs| {
            join_for_recipe(default, &value, &rest, &kwargs)
        },
    );
}

/// Encode one value as a single shell word.
///
/// The subject must be a string. `MiniJinja`'s `String` argument type would
/// stringify a number or a mapping instead, silently quoting the *rendering* of
/// a value the author meant literally — see D4.
fn quote_for_recipe(
    default: ShellDialect,
    value: &Value,
    rest: &Rest<Value>,
    kwargs: &Kwargs,
) -> Result<String, Error> {
    reject_positional("shell_quote", rest)?;
    let dialect = resolve_dialect(default, kwargs)?;
    kwargs.assert_all_used()?;
    let text = subject_as_str("shell_quote", value, keys::STDLIB_SHELL_QUOTE_NOT_STRING)?;
    encode_one(dialect, text)
}

/// Encode every member of a sequence as its own shell word.
///
/// The members are joined with a single space, so the result is one shell word
/// per member and the shell re-splits it into exactly the original sequence.
fn join_for_recipe(
    default: ShellDialect,
    value: &Value,
    rest: &Rest<Value>,
    kwargs: &Kwargs,
) -> Result<String, Error> {
    reject_positional("shell_join", rest)?;
    let dialect = resolve_dialect(default, kwargs)?;
    kwargs.assert_all_used()?;
    let kind = value.kind();
    if !matches!(kind, ValueKind::Seq | ValueKind::Iterable) {
        return Err(args_error(
            localization::message(keys::STDLIB_SHELL_JOIN_NOT_SEQUENCE)
                .with_arg("kind", kind.to_string()),
        ));
    }

    let items = value.try_iter()?;
    let mut encoded = Vec::new();
    for (index, item) in items.enumerate() {
        let Some(text) = item.as_str() else {
            return Err(args_error(
                localization::message(keys::STDLIB_SHELL_JOIN_ITEM_NOT_STRING)
                    .with_arg("index", index.to_string())
                    .with_arg("kind", item.kind().to_string()),
            ));
        };
        encoded.push(encode_one(dialect, text)?);
    }
    Ok(encoded.join(" "))
}

/// Encode one admissible string, naming the offending call site on failure.
fn encode_one(dialect: ShellDialect, text: &str) -> Result<String, Error> {
    if !shell_word::is_recipe_admissible(text) {
        return Err(unquotable_error());
    }
    Ok(shell_word::quote_word(dialect, text))
}

/// Resolve the `dialect` keyword argument, falling back to the registration
/// default.
///
/// Read as `Option<Value>` and type-checked rather than as `Option<String>`:
/// `MiniJinja`'s `String` argument type converts a number or a boolean with
/// `to_string`, so `dialect=3` would silently become the dialect named `"3"`
/// and fail as merely unknown rather than as the wrong type (D4).
///
/// The non-string case gets its own diagnostic rather than being folded into
/// [`dialect_invalid_error`]. Reporting `dialect=3` as an *unknown dialect*
/// would be true but misleading: nothing called `3` was ever a dialect name,
/// and the reader needs to know the argument's type is wrong, not that the
/// name is absent from the accepted set.
fn resolve_dialect(default: ShellDialect, kwargs: &Kwargs) -> Result<ShellDialect, Error> {
    let Some(value) = kwargs.get::<Option<Value>>("dialect")? else {
        // The omitted-dialect population is the one this counter exists to
        // make visible: its rendered text is host-dependent and unstable.
        return Ok(dialect_telemetry::record_dialect(
            default.telemetry_name(),
            dialect_telemetry::SOURCE_DEFAULT,
            default,
        ));
    };
    let Some(raw) = value.as_str() else {
        return Err(dialect_not_string_error(value.kind()));
    };
    ShellDialect::parse(raw)
        .map(|dialect| {
            dialect_telemetry::record_dialect(
                dialect.telemetry_name(),
                dialect_telemetry::SOURCE_EXPLICIT,
                dialect,
            )
        })
        .ok_or_else(|| dialect_invalid_error(&value))
}

/// Report a `dialect` argument that is not a string at all.
///
/// Separate from [`dialect_invalid_error`] so the diagnostic names the
/// received kind: a non-string never reaches the accepted-name list, and
/// telling the reader it is "unknown" would send them to check spelling.
fn dialect_not_string_error(kind: ValueKind) -> Error {
    args_error(
        localization::message(keys::STDLIB_SHELL_DIALECT_NOT_STRING)
            .with_arg("kind", kind.to_string()),
    )
}

/// Report the rejected `dialect` value and enumerate every accepted name.
fn dialect_invalid_error(value: &Value) -> Error {
    args_error(
        localization::message(keys::STDLIB_SHELL_DIALECT_INVALID)
            .with_arg("dialect", value.to_string())
            .with_arg("accepted", accepted_dialects()),
    )
}

/// Return every dialect name, in the order errors enumerate them.
fn accepted_dialects() -> String {
    ShellDialect::ALL
        .iter()
        .map(|dialect| dialect.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Reject a positional argument, which `MiniJinja` would otherwise report with a
/// bare `TooManyArguments` carrying no machine-readable code.
///
/// `dialect` is the only option either filter takes, and it is keyword-only so
/// a call site reads as self-describing. The `Rest<Value>` parameter exists
/// solely to observe the leftover positional here: without it `MiniJinja` raises
/// during argument binding, before the filter body can attach D9's code, and
/// the diagnostic loses the `[netsuke::jinja::shell::args]` prefix that the
/// localized catalogues assert on.
fn reject_positional(filter: &str, rest: &Rest<Value>) -> Result<(), Error> {
    if rest.is_empty() {
        return Ok(());
    }
    let example = format!("{filter}(dialect='{}')", ShellDialect::Sh.as_str());
    Err(args_error(
        localization::message(keys::STDLIB_SHELL_POSITIONAL_OPTION)
            .with_arg("filter", filter)
            .with_arg("example", example),
    ))
}

/// Read a filter subject as a string, naming the received kind otherwise.
///
/// `message_key` selects the filter's own wording; both variants exist so the
/// diagnostic names the filter the author actually wrote.
fn subject_as_str<'v>(
    filter: &str,
    value: &'v Value,
    message_key: &'static str,
) -> Result<&'v str, Error> {
    value.as_str().ok_or_else(|| {
        args_error(
            localization::message(message_key)
                .with_arg("filter", filter)
                .with_arg("kind", value.kind().to_string()),
        )
    })
}

/// Build the localized `args_error` wrapper around one detail message.
fn args_error(detail: impl std::fmt::Display) -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        localization::message(keys::STDLIB_SHELL_ARGS_ERROR)
            .with_arg("details", detail.to_string())
            .to_string(),
    )
}

/// Build the localized `unquotable` error wrapping the control-character rule.
fn unquotable_error() -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        localization::message(keys::STDLIB_SHELL_UNQUOTABLE)
            .with_arg(
                "details",
                localization::message(keys::STDLIB_SHELL_QUOTE_CONTROL_CHARACTER).to_string(),
            )
            .to_string(),
    )
}
