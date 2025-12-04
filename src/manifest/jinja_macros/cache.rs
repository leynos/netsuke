//! Macro caching and invocation helpers for manifest-defined Jinja macros.
use super::call_macro_value;
use anyhow::Context;
use minijinja::{
    AutoEscape, Environment, Error, ErrorKind, State,
    value::{Kwargs, Object, Rest, Value},
};
use std::{
    mem,
    ptr::NonNull,
    sync::{Arc, OnceLock},
    thread::ThreadId,
};

pub(super) fn make_macro_fn(
    cache: Arc<MacroCache>,
) -> impl Fn(&State, Rest<Value>, Kwargs) -> Result<Value, Error> {
    move |state, Rest(args), macro_kwargs| {
        let macro_instance = cache.instance().ok_or_else(|| {
            Error::new(
                ErrorKind::InvalidOperation,
                format!(
                    "macro '{}' from template '{}' must be initialised before use",
                    cache.macro_name, cache.template_name
                ),
            )
        })?;
        // MiniJinja requires keyword arguments to be appended as a trailing
        // `Kwargs` value within the positional slice. Build that value lazily so
        // we avoid allocating when no keywords were supplied.
        let mut entries: Vec<(String, Value)> = Vec::new();
        for key in macro_kwargs.args() {
            let mut value = macro_kwargs.peek::<Value>(key)?;
            if key == "caller" {
                value = adapt_caller_argument(state, value)?;
            }
            entries.push((key.to_owned(), value));
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

fn adapt_caller_argument(state: &State, value: Value) -> Result<Value, Error> {
    if value.as_object().is_some() {
        Ok(Value::from_object(CallerAdapter::new(state, value)))
    } else {
        Err(Error::new(
            ErrorKind::InvalidOperation,
            format!("'caller' argument must be callable, got {}", value.kind()),
        ))
    }
}

/// Cache of compiled macro state for repeated invocations.
#[derive(Debug)]
pub(super) struct MacroCache {
    template_name: String,
    macro_name: String,
    instance: OnceLock<MacroInstance>,
}

impl MacroCache {
    pub(super) const fn new(template_name: String, macro_name: String) -> Self {
        Self {
            template_name,
            macro_name,
            instance: OnceLock::new(),
        }
    }

    pub(super) fn prepare(&self, env: &Environment) -> anyhow::Result<()> {
        if self.instance.get().is_none() {
            let instance = MacroInstance::new(env, &self.template_name, &self.macro_name)?;
            self.instance.set(instance).ok();
        }
        Ok(())
    }

    fn instance(&self) -> Option<&MacroInstance> {
        self.instance.get()
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
    fn new(env: &Environment, template_name: &str, macro_name: &str) -> anyhow::Result<Self> {
        let template = env
            .get_template(template_name)
            .with_context(|| format!("load template '{template_name}'"))?;
        let state = template
            .eval_to_state(())
            .with_context(|| format!("initialise macro '{macro_name}'"))?;
        let value = state.lookup(macro_name).ok_or_else(|| {
            anyhow::anyhow!("macro '{macro_name}' missing from compiled template")
        })?;
        // SAFETY: `register_macro` requires an `Environment<'static>`, so the template
        // bytecode outlives the cached state stored in the macro instance.
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
/// template instructions—outlive the cached state. This matches the lifecycle
/// of manifest macros, which remain registered for the duration of the build.
#[derive(Debug)]
struct MacroStateGuard {
    ptr: NonNull<State<'static, 'static>>,
}

impl MacroStateGuard {
    fn new(state: State<'static, 'static>) -> Self {
        let boxed = Box::new(state);
        let ptr = Box::into_raw(boxed);
        // SAFETY: Box::into_raw never returns null for non-ZST types. State is
        // non-zero-sized so the pointer is guaranteed valid.
        let ptr_non_null = unsafe { NonNull::new_unchecked(ptr) };
        Self { ptr: ptr_non_null }
    }

    #[expect(
        clippy::missing_const_for_fn,
        reason = "Macro state guard relies on pointer dereferencing not supported in const contexts"
    )]
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
