//! Localization-aware value parser used to validate typed CLI arguments.
//!
//! `LocalizedValueParser` wraps a parsing closure with the active localizer
//! so that Clap's value validation can emit localized error messages. It also
//! carries optional [`PossibleValue`] metadata so the CLI adapter can advertise
//! accepted values in `--help` output without coupling the domain enums to
//! Clap (see [`super::policy::values`]).

use clap::builder::PossibleValue;
use clap::builder::TypedValueParser;
use clap::error::{ContextKind, ContextValue, ErrorKind};
use ortho_config::Localizer;
use std::sync::Arc;

/// A Clap value parser that delegates validation to a localization-aware
/// closure and optionally advertises possible values for help rendering.
#[derive(Clone)]
pub(super) struct LocalizedValueParser<F> {
    /// Shared localizer used to format validation messages.
    localizer: Arc<dyn Localizer>,
    /// Localizer-aware function validating each raw value.
    parser: F,
    /// Values advertised in `--help` for the argument being validated.
    possible_values: Option<Vec<PossibleValue>>,
}

impl<F> LocalizedValueParser<F> {
    /// Wrap `parser` with `localizer`, exposing no possible values.
    pub(super) fn new(localizer: Arc<dyn Localizer>, parser: F) -> Self {
        Self {
            localizer,
            parser,
            possible_values: None,
        }
    }

    /// Wrap `parser` with `localizer`, advertising `possible_values` in help.
    pub(super) fn with_possible_values(
        localizer: Arc<dyn Localizer>,
        parser: F,
        possible_values: Vec<PossibleValue>,
    ) -> Self {
        Self {
            localizer,
            parser,
            possible_values: Some(possible_values),
        }
    }
}

impl<F, T> TypedValueParser for LocalizedValueParser<F>
where
    F: Fn(&dyn Localizer, &str) -> Result<T, String> + Clone + Send + Sync + 'static,
    T: Send + Sync + Clone + 'static,
{
    type Value = T;

    fn parse_ref(
        &self,
        cmd: &clap::Command,
        arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        let mut command = cmd.clone();
        let Some(raw_value) = value.to_str() else {
            // The fallback must differ from the `clap-error-invalid-utf8`
            // catalogue text: `ortho_config` infers a catalogue miss by
            // comparing the resolved message with this fallback, so identical
            // wording would emit a spurious missing-translation warning even
            // when the key resolves.
            return Err(command.error(
                ErrorKind::InvalidUtf8,
                "The argument value is not valid UTF-8.",
            ));
        };
        (self.parser)(self.localizer.as_ref(), raw_value)
            .map_err(|err| value_validation_error(cmd, arg, raw_value, err))
    }

    fn possible_values(&self) -> Option<Box<dyn Iterator<Item = PossibleValue> + '_>> {
        self.possible_values
            .as_ref()
            .map(|values| Box::new(values.iter().cloned()) as Box<dyn Iterator<Item = _> + '_>)
    }
}

/// Build a `ValueValidation` error carrying the clap context that the
/// localizer needs to resolve `clap-error-value-validation`.
///
/// `Command::error` routes through `clap::Error::raw`, which attaches no
/// context at all. The catalogue message interpolates `{ $argument }` and
/// `{ $value }`, so without these entries the localizer cannot format it and
/// silently falls back to the untranslated text. `ortho_config` reads exactly
/// `InvalidArg` and `InvalidValue`, which is also what clap's own parsers
/// insert.
///
/// `arg` is `None` when clap exposes no argument to the parser. In that case
/// the argument entry is omitted rather than inventing a name; the value entry
/// is still inserted because it is always known.
fn value_validation_error(
    cmd: &clap::Command,
    arg: Option<&clap::Arg>,
    raw_value: &str,
    message: impl Into<String>,
) -> clap::Error {
    let mut command = cmd.clone();
    let mut error = command.error(ErrorKind::ValueValidation, message.into());
    if let Some(argument) = arg {
        error.insert(
            ContextKind::InvalidArg,
            ContextValue::String(argument.to_string()),
        );
    }
    error.insert(
        ContextKind::InvalidValue,
        ContextValue::String(raw_value.to_owned()),
    );
    error
}

#[cfg(test)]
mod tests {
    //! Unit tests for the clap context attached by `LocalizedValueParser`.

    use super::*;

    /// Reject every value, simulating any failing validation closure.
    fn reject(_localizer: &dyn Localizer, _raw: &str) -> Result<String, String> {
        Err("rejected by test parser".to_owned())
    }

    /// Build an English localizer for the parser under test.
    fn localizer() -> Arc<dyn Localizer> {
        Arc::from(crate::cli_localization::build_localizer(Some("en-US")))
    }

    /// Build a rejecting parser over the English localizer.
    fn parser() -> LocalizedValueParser<
        impl Fn(&dyn Localizer, &str) -> Result<String, String> + Clone + Send + Sync + 'static,
    > {
        LocalizedValueParser::new(localizer(), reject)
    }

    #[test]
    fn parse_ref_inserts_invalid_arg_and_value_context() {
        let command = clap::Command::new("probe");
        let arg = clap::Arg::new("color")
            .long("color")
            .value_name("POLICY")
            .num_args(1);
        let error = parser()
            .parse_ref(&command, Some(&arg), std::ffi::OsStr::new("bogus"))
            .expect_err("rejecting parser should fail");

        assert_eq!(error.kind(), ErrorKind::ValueValidation);
        assert_eq!(
            error.get(ContextKind::InvalidArg),
            Some(&ContextValue::String("--color <POLICY>".to_owned())),
            "argument context should render the clap display form"
        );
        assert_eq!(
            error.get(ContextKind::InvalidValue),
            Some(&ContextValue::String("bogus".to_owned())),
            "value context should carry the raw input"
        );
    }

    #[test]
    fn parse_ref_without_arg_omits_argument_context_but_keeps_value() {
        let command = clap::Command::new("probe");
        let error = parser()
            .parse_ref(&command, None, std::ffi::OsStr::new("bogus"))
            .expect_err("rejecting parser should fail");

        assert_eq!(error.kind(), ErrorKind::ValueValidation);
        assert!(
            error.get(ContextKind::InvalidArg).is_none(),
            "no argument name should be invented when clap supplies none"
        );
        assert_eq!(
            error.get(ContextKind::InvalidValue),
            Some(&ContextValue::String("bogus".to_owned())),
        );
        assert!(
            error.to_string().contains("rejected by test parser"),
            "the closure text should remain the raw fallback: {error}"
        );
    }

    /// Non-UTF-8 input must fail before the validation closure sees it, and
    /// the diagnostic still needs a catalogue key (`clap-error-invalid-utf8`)
    /// because the localizer renders by error kind.
    #[cfg(unix)]
    #[test]
    fn parse_ref_rejects_non_utf8_without_invoking_the_parser() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        use std::sync::atomic::{AtomicBool, Ordering};

        let invoked = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&invoked);
        let parser = LocalizedValueParser::new(
            localizer(),
            move |_: &dyn Localizer, _: &str| -> Result<String, String> {
                flag.store(true, Ordering::SeqCst);
                Ok(String::new())
            },
        );

        let error = parser
            .parse_ref(
                &clap::Command::new("probe"),
                None,
                OsStr::from_bytes(b"manifest-\xff"),
            )
            .expect_err("non-UTF-8 input should fail");

        assert_eq!(error.kind(), ErrorKind::InvalidUtf8);
        assert!(
            !invoked.load(Ordering::SeqCst),
            "the validation closure must not run for non-UTF-8 input"
        );
    }
}
