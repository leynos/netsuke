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
use serde_json::Value as YamlValue;
use std::{
    mem,
    ptr::NonNull,
    sync::{Arc, OnceLock},
    thread::ThreadId,
};

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

    let cache = Arc::new(MacroCache::new(template_name, name.clone()));
    cache.prepare(env)?;
    env.add_function(name.clone(), make_macro_fn(cache));
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

    let defs: Vec<MacroDefinition> = serde_json::from_value(macros)
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
pub(crate) fn call_macro_value(
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
/// The returned closure captures the compiled macro [`Value`] and reuses it for
/// every invocation. This avoids re-parsing the manifest macro template for each
/// call while still evaluating keyword arguments lazily.
///
/// # Examples
///
/// ```rust,ignore
/// # use minijinja::Environment;
/// # use minijinja::value::{Kwargs, Rest, Value};
/// # use std::sync::Arc;
/// # use netsuke::manifest::jinja_macros::make_macro_fn;
/// let mut env = Environment::new();
/// env.add_template(
///     "macro",
///     "{% macro greet(name='friend') %}hi {{ name }}{% endmacro %}",
/// )
/// .unwrap();
/// let template = env.get_template("macro").unwrap();
/// let state = template.eval_to_state(()).unwrap();
/// let cache = Arc::new(MacroCache::new("macro".into(), "greet".into()));
/// cache.prepare(&env).unwrap();
/// let wrapper = make_macro_fn(Arc::clone(&cache));
/// let kwargs = Kwargs::from_iter([(String::from("name"), Value::from("Ada"))]);
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
    cache: Arc<MacroCache>,
) -> impl Fn(&State, Rest<Value>, Kwargs) -> Result<Value, Error> {
    move |state, Rest(args), kwargs| {
        let macro_instance = cache.instance();
        // MiniJinja requires keyword arguments to be appended as a trailing
        // `Kwargs` value within the positional slice. Build that value lazily so
        // we avoid allocating when no keywords were supplied.
        let mut entries: Vec<(String, Value)> = Vec::new();
        for key in kwargs.args() {
            let mut value = kwargs.peek::<Value>(key)?;
            if key == "caller" {
                if value.as_object().is_some() {
                    value = Value::from_object(CallerAdapter::new(state, value));
                } else {
                    return Err(Error::new(
                        ErrorKind::InvalidOperation,
                        format!("'caller' argument must be callable, got {}", value.kind()),
                    ));
                }
            }
            entries.push((key.to_string(), value));
        }
        let maybe_kwargs = if entries.is_empty() {
            None
        } else {
            Some(entries.into_iter().collect::<Kwargs>())
        };

        debug_assert_eq!(
            std::thread::current().id(),
            macro_instance.owner_thread,
            "manifest macro invoked on a different thread"
        );
        let rendered_value = call_macro_value(
            macro_instance.state.as_ref(),
            &macro_instance.value,
            &args,
            maybe_kwargs,
        )?;
        let rendered: String = rendered_value.into();
        let value = if matches!(state.auto_escape(), AutoEscape::None) {
            Value::from(rendered)
        } else {
            Value::from_safe_string(rendered)
        };
        Ok(value)
    }
}

/// Cache of compiled macro state for repeated invocations.
#[derive(Debug)]
struct MacroCache {
    template_name: String,
    macro_name: String,
    instance: OnceLock<MacroInstance>,
}

impl MacroCache {
    fn new(template_name: String, macro_name: String) -> Self {
        Self {
            template_name,
            macro_name,
            instance: OnceLock::new(),
        }
    }

    fn prepare(&self, env: &Environment) -> Result<()> {
        if self.instance.get().is_none() {
            let instance = MacroInstance::new(env, &self.template_name, &self.macro_name)?;
            let _ = self.instance.set(instance);
        }
        Ok(())
    }

    fn instance(&self) -> &MacroInstance {
        self.instance
            .get()
            .expect("macro instance must be initialised before use")
    }
}

/// Retains the compiled macro and its backing state for reuse.
///
/// # Thread Safety Notice
///
/// The cached state is initialised on the registering thread and relies on the
/// engine invoking the macro on that same thread. The [`Send`] and [`Sync`]
/// implementations exist to satisfy `MiniJinja`'s requirements for registered
/// functions, but the runtime asserts that calls remain on the original thread
/// in debug builds.
#[derive(Debug)]
struct MacroInstance {
    state: MacroStateGuard,
    value: Value,
    owner_thread: ThreadId,
}

impl MacroInstance {
    fn new(env: &Environment, template_name: &str, macro_name: &str) -> Result<Self> {
        let template = env
            .get_template(template_name)
            .with_context(|| format!("load template '{template_name}'"))?;
        let state = template
            .eval_to_state(())
            .with_context(|| format!("initialise macro '{macro_name}'"))?;
        let value = state.lookup(macro_name).ok_or_else(|| {
            anyhow::anyhow!("macro '{macro_name}' missing from compiled template")
        })?;
        // SAFETY: manifest macros are registered in an `Environment<'static>` so the
        // template bytecode outlives the cache.
        let state_static: State<'static, 'static> = unsafe { mem::transmute(state) };
        Ok(Self {
            state: MacroStateGuard::new(state_static),
            value,
            owner_thread: std::thread::current().id(),
        })
    }
}

unsafe impl Send for MacroInstance {}
unsafe impl Sync for MacroInstance {}

/// Owning handle for the compiled [`State`] used when invoking a macro.
///
/// The boxed state gives the macro a stable context across repeated calls while
/// keeping the allocation scoped to the cache lifetime.
///
/// # Safety
///
/// The guard assumes the manifest environment—and therefore the compiled
/// template instructions—outlive the cached state. This matches the lifecycle of
/// manifest macros, which remain registered for the duration of the build.
#[derive(Debug)]
struct MacroStateGuard {
    ptr: NonNull<State<'static, 'static>>,
}

impl MacroStateGuard {
    fn new(state: State<'static, 'static>) -> Self {
        let boxed = Box::new(state);
        let ptr = NonNull::new(Box::into_raw(boxed)).expect("macro state pointer");
        Self { ptr }
    }

    fn as_ref(&self) -> &State<'static, 'static> {
        unsafe { self.ptr.as_ref() }
    }
}

impl Drop for MacroStateGuard {
    fn drop(&mut self) {
        unsafe { drop(Box::from_raw(self.ptr.as_ptr())) }
    }
}

unsafe impl Send for MacroStateGuard {}
unsafe impl Sync for MacroStateGuard {}

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
/// # Thread Safety Notice
///
/// `CallerAdapter` is marked as `Send` and `Sync` via unsafe implementations.
/// This mirrors `MiniJinja`'s own macro helpers so the value can cross thread
/// boundaries when stored inside [`Value`]. The underlying pointer to [`State`]
/// is not synchronised and therefore must only be used from the thread that
/// created the adapter. Moving the adapter to other threads may trigger
/// undefined behaviour.
///
/// ## Usage Restrictions
///
/// - Only construct the adapter with states that outlive the macro invocation.
/// - Never mutate the referenced [`State`] concurrently.
/// - Avoid sending the adapter across threads; the debug assertions in
///   [`Object::call`] will catch accidental misuse during development, but
///   release builds rely on disciplined usage.
#[derive(Debug)]
struct CallerAdapter {
    caller: Value,
    state: NonNull<State<'static, 'static>>,
    owner_thread: ThreadId,
}

impl CallerAdapter {
    fn new(state: &State, caller: Value) -> Self {
        let ptr = NonNull::from(state).cast::<State<'static, 'static>>();
        Self {
            caller,
            state: ptr,
            owner_thread: std::thread::current().id(),
        }
    }
}

unsafe impl Send for CallerAdapter {}
unsafe impl Sync for CallerAdapter {}

impl Object for CallerAdapter {
    fn call(self: &Arc<Self>, _state: &State, args: &[Value]) -> Result<Value, Error> {
        debug_assert_eq!(
            std::thread::current().id(),
            self.owner_thread,
            "CallerAdapter used from a different thread"
        );
        let state = unsafe { self.state.as_ref() };
        self.caller.call(state, args)
    }
}
