//! Helpers for registering and invoking manifest-defined Jinja macros.
//!
//! The manifest can define reusable macros via the `macros` section. This module
//! compiles those macros into standalone templates and exposes them to the main
//! rendering environment so manifest templates can invoke them like built-in
//! helpers.

use crate::ast::MacroDefinition;
use anyhow::{Context, Result};
use minijinja::{
    AutoEscape, Environment, Error, ErrorKind, State,
    value::{Kwargs, Object, Rest, Value},
};
use serde_yml::Value as YamlValue;
use std::{ptr, sync::Arc};

/// Extract the macro identifier from a signature string.
///
/// The signature must follow the form `name(params)` where `name` is a valid
/// Jinja identifier and `params` is a parameter list (possibly empty).
///
/// # Errors
///
/// Returns an error if the signature is empty, lacks a parameter list, or the
/// identifier before `(` is empty.
///
/// # Examples
///
/// ```rust,ignore
/// let name = parse_macro_name("greet(name)").expect("valid signature");
/// assert_eq!(name, "greet");
/// ```
pub(crate) fn parse_macro_name(signature: &str) -> Result<String> {
    let trimmed = signature.trim();
    if trimmed.is_empty() {
        return Err(anyhow::anyhow!(
            "macro signature '{signature}' is missing an identifier"
        ));
    }
    let Some((name, _rest)) = trimmed.split_once('(') else {
        return Err(anyhow::anyhow!(
            "macro signature '{signature}' must include parameter list"
        ));
    };
    let name = name.trim();
    if name.is_empty() {
        return Err(anyhow::anyhow!(
            "macro signature '{signature}' is missing an identifier"
        ));
    }
    Ok(name.to_string())
}

/// Register a single manifest macro in the Jinja environment.
///
/// Compiles the macro body into a template and registers a callable function
/// with the extracted macro name. The template name is synthesised using the
/// provided index to ensure uniqueness.
///
/// # Errors
///
/// Returns an error if the macro signature is invalid or template compilation
/// fails.
pub(crate) fn register_macro(
    env: &mut Environment,
    macro_def: &MacroDefinition,
    index: usize,
) -> Result<()> {
    let name = parse_macro_name(&macro_def.signature)?;
    let template_name = format!("__manifest_macro_{index}_{name}");
    let template_source = format!(
        "{{% macro {} %}}{}{{% endmacro %}}",
        macro_def.signature, macro_def.body
    );

    env.add_template_owned(template_name.clone(), template_source)
        .with_context(|| format!("compile macro '{name}'"))?;

    env.add_function(name.clone(), make_macro_fn(template_name, name));
    Ok(())
}

/// Register all manifest macros from a YAML document.
///
/// Expects the YAML to have a `macros` key containing a sequence of mappings,
/// each with `signature` and `body` string fields. Registers each macro in the
/// environment using [`register_macro`].
///
/// # Errors
///
/// Returns an error if the YAML shape is invalid, any macro signature is
/// malformed, or template compilation fails.
pub(crate) fn register_manifest_macros(doc: &YamlValue, env: &mut Environment) -> Result<()> {
    let Some(macros) = doc.get("macros").cloned() else {
        return Ok(());
    };

    let defs: Vec<MacroDefinition> = serde_yml::from_value(macros)
        .context("macros must be a sequence of mappings with string signature/body")?;

    for (idx, def) in defs.iter().enumerate() {
        register_macro(env, def, idx)
            .with_context(|| format!("register macro '{}'", def.signature))?;
    }
    Ok(())
}

/// Invoke a `MiniJinja` value with optional keyword arguments.
///
/// `MiniJinja` encodes keyword arguments by appending a [`Kwargs`] value to the
/// positional slice. This helper hides that convention so callers can pass the
/// keyword collection explicitly.
///
/// # Examples
///
/// ```rust,ignore
/// use minijinja::{Environment, value::{Kwargs, Value}};
/// use netsuke::manifest::jinja_macros::call_macro_value;
///
/// let mut env = Environment::new();
/// env.add_template(
///     "macro",
///     "{% macro greet(name='friend') %}hi {{ name }}{% endmacro %}",
/// )
/// .unwrap();
/// let template = env.get_template("macro").unwrap();
/// let state = template.eval_to_state(()).unwrap();
/// let value = state.lookup("greet").unwrap();
/// let kwargs = Kwargs::from_iter([(String::from("name"), Value::from("Ada"))]);
/// let rendered = call_macro_value(&state, &value, &[], Some(kwargs)).unwrap();
/// assert_eq!(rendered.to_string(), "hi Ada");
/// ```
fn call_macro_value(
    state: &State,
    macro_value: &Value,
    positional: &[Value],
    kwargs: Option<Kwargs>,
) -> Result<Value, Error> {
    kwargs.map_or_else(
        || macro_value.call(state, positional),
        |kwargs| {
            let mut call_args = Vec::with_capacity(positional.len() + 1);
            call_args.extend_from_slice(positional);
            call_args.push(Value::from(kwargs));
            macro_value.call(state, call_args.as_slice())
        },
    )
}

/// Create a wrapper that invokes a compiled manifest macro on demand.
///
/// The returned closure fetches the internal template, resolves the macro, and
/// forwards the provided positional and keyword arguments to the call.
///
/// # Examples
///
/// ```rust,ignore
/// # use minijinja::Environment;
/// # use minijinja::value::{Kwargs, Rest, Value};
/// # use netsuke::manifest::jinja_macros::make_macro_fn;
/// let mut env = Environment::new();
/// env.add_template(
///     "macro",
///     "{% macro greet(name='friend') %}hi {{ name }}{% endmacro %}",
/// )
/// .unwrap();
/// let wrapper = make_macro_fn("macro".into(), "greet".into());
/// let state = env
///     .get_template("macro")
///     .unwrap()
///     .eval_to_state(())
///     .unwrap();
/// let kwargs = Kwargs::from_iter([
///     (String::from("name"), Value::from("Ada")),
/// ]);
/// let output = wrapper(&state, Rest(vec![]), kwargs)
///     .unwrap()
///     .to_string();
/// assert_eq!(output, "hi Ada");
/// ```
///
/// # Errors
///
/// The wrapper returns an error if the macro cannot be located or execution
/// fails.
fn make_macro_fn(
    template_name: String,
    macro_name: String,
) -> impl Fn(&State, Rest<Value>, Kwargs) -> Result<Value, Error> {
    move |state, Rest(args), kwargs| {
        let template = state.env().get_template(&template_name)?;
        let macro_state = template.eval_to_state(())?;
        let macro_value = macro_state.lookup(&macro_name).ok_or_else(|| {
            Error::new(
                ErrorKind::InvalidOperation,
                format!("macro '{macro_name}' not defined in '{template_name}'"),
            )
        })?;

        // MiniJinja requires keyword arguments to be appended as a trailing
        // `Kwargs` value within the positional slice. Build that value lazily so
        // we avoid allocating when no keywords were supplied.
        let mut entries: Vec<(String, Value)> = Vec::new();
        for key in kwargs.args() {
            let mut value = kwargs.peek::<Value>(key)?;
            if key == "caller" {
                value = Value::from_object(CallerAdapter::new(state, value));
            }
            entries.push((key.to_string(), value));
        }
        let maybe_kwargs = if entries.is_empty() {
            None
        } else {
            Some(entries.into_iter().collect::<Kwargs>())
        };

        let rendered_value = call_macro_value(&macro_state, &macro_value, &args, maybe_kwargs)?;
        let rendered: String = rendered_value.into();
        let value = if matches!(state.auto_escape(), AutoEscape::None) {
            Value::from(rendered)
        } else {
            Value::from_safe_string(rendered)
        };
        Ok(value)
    }
}

/// Adapter to preserve the outer template state for caller block invocation.
///
/// `MiniJinja` executes the wrapper closure with a synthetic [`State`] that
/// differs from the one active when the manifest macro was called. Caller
/// blocks, however, must run within the original context to access the
/// manifest's globals and captured variables. This adapter stores a raw
/// pointer to that state so the macro can invoke the block later.
///
/// # Safety
///
/// The raw pointer is only valid while the outer [`State`] is alive. Safety
/// hinges on:
///
/// - the macro invocation remaining synchronous (no `async` suspension)
/// - the original state outliving every [`CallerAdapter`] invocation
/// - the adapter not moving across threads despite the `Send`/`Sync` impls
///
/// The unsynchronised `Send` and `Sync` impls mirror `MiniJinja`'s built-in
/// macro objects. They rely on the engine executing caller blocks on the same
/// thread that created the adapter, which matches the runtime's behaviour.
#[derive(Debug)]
struct CallerAdapter {
    caller: Value,
    state: *const State<'static, 'static>,
}

impl CallerAdapter {
    fn new(state: &State, caller: Value) -> Self {
        let ptr = ptr::from_ref(state).cast::<State<'static, 'static>>();
        Self { caller, state: ptr }
    }
}

unsafe impl Send for CallerAdapter {}
unsafe impl Sync for CallerAdapter {}

impl Object for CallerAdapter {
    fn call(self: &Arc<Self>, _state: &State, args: &[Value]) -> Result<Value, Error> {
        let state = unsafe { &*self.state };
        self.caller.call(state, args)
    }
}
