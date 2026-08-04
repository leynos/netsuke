//! Safe invocation helpers for manifest-defined Jinja macros.

use super::call_macro_value;
use crate::localization::{self, keys};
use minijinja::{
    AutoEscape, Captured, Environment, Error, ErrorKind, State,
    value::{Kwargs, Rest, Value},
};

/// Build the global-function fallback used by compiled Jinja expressions.
///
/// Template rendering imports manifest macros into the active template state,
/// which is the path that supports Jinja call blocks. Compiled expressions do
/// not support imports, so this fallback creates a short-lived captured state
/// for each expression call instead of extending its lifetime unsafely.
pub(super) fn make_macro_fn(
    template_name: String,
    macro_name: String,
) -> impl Fn(&State, Rest<Value>, Kwargs) -> Result<Value, Error> {
    move |state, Rest(args), macro_kwargs| {
        let (captured, macro_value) = capture_macro(state.env(), &template_name, &macro_name)?;
        let maybe_kwargs = collect_kwargs(&macro_kwargs)?;
        let rendered_value = call_macro_value(
            captured.state(),
            &macro_value,
            args.as_slice(),
            maybe_kwargs,
        )?;
        let rendered: String = rendered_value.into();
        Ok(if matches!(state.auto_escape(), AutoEscape::None) {
            Value::from(rendered)
        } else {
            Value::from_safe_string(rendered)
        })
    }
}

/// Confirm that a compiled template exports the requested macro.
pub(super) fn validate_macro(
    env: &Environment,
    template_name: &str,
    macro_name: &str,
) -> anyhow::Result<()> {
    capture_macro(env, template_name, macro_name)?;
    Ok(())
}

fn capture_macro<'source>(
    env: &'source Environment<'source>,
    template_name: &str,
    macro_name: &str,
) -> Result<(Captured<'source>, Value), Error> {
    let template = env.get_template(template_name).map_err(|error| {
        Error::new(
            error.kind(),
            localization::message(keys::MANIFEST_MACRO_TEMPLATE_LOAD_FAILED)
                .with_arg("template", template_name)
                .to_string(),
        )
        .with_source(error)
    })?;
    let captured = template.render_captured(()).map_err(|error| {
        Error::new(
            error.kind(),
            localization::message(keys::MANIFEST_MACRO_INIT_FAILED)
                .with_arg("macro", macro_name)
                .to_string(),
        )
        .with_source(error)
    })?;
    let value = captured.state().lookup(macro_name).ok_or_else(|| {
        Error::new(
            ErrorKind::InvalidOperation,
            localization::message(keys::MANIFEST_MACRO_MISSING)
                .with_arg("macro", macro_name)
                .to_string(),
        )
    })?;
    Ok((captured, value))
}

fn collect_kwargs(macro_kwargs: &Kwargs) -> Result<Option<Kwargs>, Error> {
    let mut entries = Vec::new();
    for key in macro_kwargs.args() {
        entries.push((key.to_owned(), macro_kwargs.peek::<Value>(key)?));
    }
    Ok((!entries.is_empty()).then(|| entries.into_iter().collect()))
}
