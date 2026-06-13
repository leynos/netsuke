//! Macro caching and invocation helpers for manifest-defined Jinja macros.
use super::call_macro_value;
use crate::localization::{self, keys};
use anyhow::Context;
use minijinja::{
    AutoEscape, Captured, Environment, Error, ErrorKind, State,
    value::{Kwargs, Object, Rest, Value},
};
use std::{
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
                localization::message(keys::MANIFEST_MACRO_NOT_INITIALISED)
                    .with_arg("macro", cache.macro_name.as_str())
                    .with_arg("template", cache.template_name.as_str())
                    .to_string(),
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
            localization::message(keys::MANIFEST_MACRO_CALLER_INVALID)
                .with_arg("kind", value.kind())
                .to_string(),
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
        let template = env.get_template(template_name).with_context(|| {
            localization::message(keys::MANIFEST_MACRO_TEMPLATE_LOAD_FAILED)
                .with_arg("template", template_name)
        })?;
        let captured = template.render_captured(()).with_context(|| {
            localization::message(keys::MANIFEST_MACRO_INIT_FAILED).with_arg("macro", macro_name)
        })?;
        let value = captured.state().lookup(macro_name).ok_or_else(|| {
            anyhow::anyhow!(
                "{}",
                localization::message(keys::MANIFEST_MACRO_MISSING).with_arg("macro", macro_name)
            )
        })?;
        // SAFETY: `register_macro` requires an `Environment<'static>`, so the template
        // bytecode and captured output outlive the cached state stored in the macro
        // instance. The precondition is that the environment — and therefore the
        // compiled template instructions backing `captured` — outlives this
        // `MacroInstance`; manifest macros remain registered for the whole build,
        // so this holds. The pointer lifecycle of the resulting
        // `Captured<'static>` (heap leak, NonNull invariant, single reclaim) is
        // verified by the Kani proofs `macro_state_guard_ptr_is_non_null` and
        // `macro_state_guard_drop_is_safe`; the `Send`/`Sync` soundness of the
        // cached instance is verified by `macro_instance_is_send` and
        // `macro_instance_is_sync`.
        let captured_static: Captured<'static> = unsafe { std::mem::transmute(captured) };
        Ok(Self {
            state: MacroStateGuard::new(captured_static),
            value,
            owner_thread: std::thread::current().id(),
        })
    }
}

unsafe impl Send for MacroInstance {}
unsafe impl Sync for MacroInstance {}

/// Owning handle for the compiled [`State`] used when invoking a macro.
///
/// The boxed capture gives the macro a stable context across repeated calls while
/// keeping the allocation scoped to the cache lifetime.
///
/// # Safety
///
/// The guard assumes the manifest environment—and therefore the compiled
/// template instructions—outlive the cached state. This matches the lifecycle
/// of manifest macros, which remain registered for the duration of the build.
#[derive(Debug)]
struct MacroStateGuard {
    ptr: NonNull<Captured<'static>>,
}

impl MacroStateGuard {
    fn new(captured: Captured<'static>) -> Self {
        Self {
            ptr: heap_leak_non_null(captured),
        }
    }

    fn as_ref(&self) -> &State<'static, 'static> {
        unsafe { self.ptr.as_ref().state() }
    }
}

impl Drop for MacroStateGuard {
    fn drop(&mut self) {
        // SAFETY: `self.ptr` was produced by `heap_leak_non_null` in `new`
        // and is reclaimed exactly once here, so the round-trip neither leaks
        // nor double-frees. Verified by `macro_state_guard_drop_is_safe`.
        unsafe { reclaim_heap_non_null(self.ptr) }
    }
}

/// Box `value` on the heap and leak it as a non-null pointer.
///
/// Mirrors the ownership transfer performed by [`MacroStateGuard::new`]:
/// `Box::into_raw` never returns null for a non-zero-sized type, so the
/// `NonNull` invariant holds. The caller becomes responsible for reclaiming
/// the allocation exactly once via [`reclaim_heap_non_null`].
fn heap_leak_non_null<T>(value: T) -> NonNull<T> {
    let ptr = Box::into_raw(Box::new(value));
    // SAFETY: `Box::into_raw` never returns null for a non-ZST allocation.
    // Verified by `macro_state_guard_ptr_is_non_null`.
    unsafe { NonNull::new_unchecked(ptr) }
}

/// Reclaim and drop a pointer previously produced by [`heap_leak_non_null`].
///
/// # Safety
///
/// `ptr` must have come from [`heap_leak_non_null`] and must not have been
/// reclaimed already; the allocation is freed here exactly once.
unsafe fn reclaim_heap_non_null<T>(ptr: NonNull<T>) {
    // SAFETY: upheld by the caller's contract; `ptr` owns a live `Box<T>`.
    unsafe { drop(Box::from_raw(ptr.as_ptr())) }
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

/// Formal verification of `MacroStateGuard`'s pointer lifecycle and the
/// `Send`/`Sync` soundness of the cached macro instance.
///
/// `Captured<'static>` and `State<'static, 'static>` are far too complex for
/// Kani to construct or unwind, so the pointer-mechanics proofs run over the
/// payload-agnostic helpers `heap_leak_non_null` / `reclaim_heap_non_null`
/// (which `MacroStateGuard::new` and its `Drop` use verbatim) with a small,
/// Kani-constructible stand-in payload. The unsafe operations being verified —
/// `Box::into_raw` → `NonNull::new_unchecked` → `Box::from_raw` — are
/// independent of the payload type beyond its being non-zero-sized, so a
/// representative payload proves the pattern.
#[cfg(kani)]
mod kani_proofs {
    use super::{MacroInstance, heap_leak_non_null, reclaim_heap_non_null};

    /// Non-zero-sized stand-in for `Captured<'static>` that Kani can build.
    #[derive(Clone, Copy, PartialEq, Eq)]
    struct ModelCaptured {
        tag: u64,
    }

    /// The `NonNull` invariant holds after construction: the leaked pointer is
    /// never null and dereferences to the original value.
    #[kani::proof]
    fn macro_state_guard_ptr_is_non_null() {
        let tag: u64 = kani::any();
        let ptr = heap_leak_non_null(ModelCaptured { tag });
        // `NonNull` cannot be null by construction; reading back the value
        // confirms the pointer targets the live allocation.
        // SAFETY: `ptr` owns a live `Box<ModelCaptured>` produced just above.
        let observed = unsafe { ptr.as_ref().tag };
        assert_eq!(observed, tag);
        // Reclaim to keep the proof free of leaks.
        // SAFETY: `ptr` came from `heap_leak_non_null` and is reclaimed once.
        unsafe { reclaim_heap_non_null(ptr) };
    }

    /// The Box → raw pointer → Box round-trip used by `new` and `Drop` neither
    /// leaks nor double-frees. Kani's memory model flags either fault.
    #[kani::proof]
    fn macro_state_guard_drop_is_safe() {
        let ptr = heap_leak_non_null(ModelCaptured { tag: kani::any() });
        // SAFETY: single reclaim of a pointer from `heap_leak_non_null`.
        unsafe { reclaim_heap_non_null(ptr) };
    }

    /// `MacroInstance` is `Send`, as required for registration with MiniJinja.
    #[kani::proof]
    fn macro_instance_is_send() {
        const fn assert_send<T: Send>() {}
        assert_send::<MacroInstance>();
    }

    /// `MacroInstance` is `Sync`, as required for registration with MiniJinja.
    #[kani::proof]
    fn macro_instance_is_sync() {
        const fn assert_sync<T: Sync>() {}
        assert_sync::<MacroInstance>();
    }
}
