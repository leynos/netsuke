//! Localization-aware value parser used to validate typed CLI arguments.
//!
//! `LocalizedValueParser` wraps a parsing closure with the active localizer
//! so that Clap's value validation can emit localized error messages. It also
//! carries optional [`PossibleValue`] metadata so the CLI adapter can advertise
//! accepted values in `--help` output without coupling the domain enums to
//! Clap (see [`super::policy_values`]).

use clap::builder::PossibleValue;
use clap::builder::TypedValueParser;
use clap::error::ErrorKind;
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
        _arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        let mut command = cmd.clone();
        let Some(raw_value) = value.to_str() else {
            return Err(command.error(ErrorKind::InvalidUtf8, "invalid UTF-8"));
        };
        (self.parser)(self.localizer.as_ref(), raw_value)
            .map_err(|err| command.error(ErrorKind::ValueValidation, err))
    }

    fn possible_values(&self) -> Option<Box<dyn Iterator<Item = PossibleValue> + '_>> {
        self.possible_values
            .as_ref()
            .map(|values| Box::new(values.iter().cloned()) as Box<dyn Iterator<Item = _> + '_>)
    }
}
