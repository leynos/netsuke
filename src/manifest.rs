//! Manifest loading helpers.
//!
//! This module parses a `Netsukefile` without relying on a global Jinja
//! preprocessing pass. The YAML is parsed first and Jinja expressions are
//! evaluated only within string values or the `foreach` and `when` keys. It
//! exposes `env()` to read environment variables and `glob()` to expand
//! filesystem patterns during template evaluation. Both helpers fail fast when
//! inputs are missing or patterns are invalid.

use crate::ast::{MacroDefinition, NetsukeManifest};
use anyhow::{Context, Result};
use minijinja::{
    AutoEscape, Environment, Error, ErrorKind, State, UndefinedBehavior,
    value::{Kwargs, Object, Rest, Value},
};
use serde_yml::Value as YamlValue;
use std::{fs, path::Path, ptr, sync::Arc};

/// A display name for a manifest source, used in error reporting.
#[derive(Debug, Clone)]
pub struct ManifestName(String);

impl ManifestName {
    /// Construct a manifest name for diagnostics.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use netsuke::manifest::ManifestName;
    /// let name = ManifestName::new("Netsukefile");
    /// assert_eq!(name.to_string(), "Netsukefile");
    /// ```
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    #[must_use]
    /// Borrow the manifest name as a string slice.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use netsuke::manifest::ManifestName;
    /// let name = ManifestName::new("Config");
    /// assert_eq!(name.as_str(), "Config");
    /// ```
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ManifestName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for ManifestName {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

mod diagnostics;
mod expand;
mod glob;
mod hints;
mod render;

pub use diagnostics::{ManifestError, map_yaml_error};
pub use glob::glob_paths;

pub use expand::expand_foreach;
pub use render::render_manifest;

/// Resolve the value of an environment variable for the `env()` Jinja helper.
///
/// Returns the variable's value or a structured error that mirrors Jinja's
/// failure modes, ensuring templates halt when a variable is missing or not
/// valid UTF-8.
///
/// # Examples
///
/// The [`EnvLock`](test_support::env_lock::EnvLock) guard serialises access to
/// the process environment so tests do not interfere with each other.
///
/// ```rust,ignore
/// use test_support::env_lock::EnvLock;
/// let _guard = EnvLock::acquire();
/// std::env::set_var("FOO", "bar");
/// assert_eq!(env("FOO").unwrap(), "bar");
/// std::env::remove_var("FOO");
/// ```
fn env_var(name: &str) -> std::result::Result<String, Error> {
    match std::env::var(name) {
        Ok(val) => Ok(val),
        Err(std::env::VarError::NotPresent) => Err(Error::new(
            ErrorKind::UndefinedError,
            format!("environment variable '{name}' is not set"),
        )),
        Err(std::env::VarError::NotUnicode(_)) => Err(Error::new(
            ErrorKind::InvalidOperation,
            format!("environment variable '{name}' is set but contains invalid UTF-8"),
        )),
    }
}

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
fn parse_macro_name(signature: &str) -> Result<String> {
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
fn register_macro(env: &mut Environment, macro_def: &MacroDefinition, index: usize) -> Result<()> {
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
fn register_manifest_macros(doc: &YamlValue, env: &mut Environment) -> Result<()> {
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
/// use netsuke::manifest::call_macro_value;
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
/// # use netsuke::manifest::make_macro_fn;
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

/// Parse a manifest string using Jinja for value templating.
///
/// The input YAML must be valid on its own. Jinja expressions are evaluated
/// only inside recognised string fields and the `foreach` and `when` keys.
///
/// # Errors
///
/// Returns an error if YAML parsing or Jinja evaluation fails.
fn from_str_named(yaml: &str, name: &ManifestName) -> Result<NetsukeManifest> {
    let mut doc: YamlValue = serde_yml::from_str(yaml).map_err(|e| ManifestError::Parse {
        source: map_yaml_error(e, yaml, name.as_ref()),
    })?;

    let mut jinja = Environment::new();
    jinja.set_undefined_behavior(UndefinedBehavior::Strict);
    // Expose custom helpers to templates.
    jinja.add_function("env", |name: String| env_var(&name));
    jinja.add_function("glob", |pattern: String| glob_paths(&pattern));
    let _stdlib_state = crate::stdlib::register(&mut jinja);

    if let Some(vars) = doc.get("vars").and_then(|v| v.as_mapping()).cloned() {
        for (k, v) in vars {
            let key = k
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("non-string key in vars mapping: {k:?}"))?
                .to_string();
            jinja.add_global(key, Value::from_serialize(v));
        }
    }

    register_manifest_macros(&doc, &mut jinja)?;

    expand_foreach(&mut doc, &jinja)?;

    let manifest: NetsukeManifest =
        serde_yml::from_value(doc).map_err(|e| ManifestError::Parse {
            source: map_yaml_error(e, yaml, name.as_ref()),
        })?;

    render_manifest(manifest, &jinja)
}

/// Parse a manifest string using Jinja for value templating.
///
/// The input YAML must be valid on its own. Jinja expressions are evaluated
/// only inside recognised string fields and the `foreach` and `when` keys.
///
/// # Errors
///
/// Returns an error if YAML parsing or Jinja evaluation fails.
pub fn from_str(yaml: &str) -> Result<NetsukeManifest> {
    from_str_named(yaml, &ManifestName::new("Netsukefile"))
}

/// Load a [`NetsukeManifest`] from the given file path.
///
/// # Errors
///
/// Returns an error if the file cannot be read or the YAML fails to parse.
pub fn from_path(path: impl AsRef<Path>) -> Result<NetsukeManifest> {
    let path_ref = path.as_ref();
    let data = fs::read_to_string(path_ref)
        .with_context(|| format!("failed to read {}", path_ref.display()))?;
    let name = ManifestName::new(path_ref.display().to_string());
    from_str_named(&data, &name)
}

#[cfg(test)]
mod tests;
