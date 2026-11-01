//! Test-supplied template functions that stand in for withheld helpers.
//!
//! A `netsuke test` load is deny-by-default: a helper that reaches the host is
//! registered as a refusal, so a manifest that calls one fails. Overlays let a
//! test supply the answer a run on a real machine would have produced, without
//! granting the helper itself. An overlay is a template function registered
//! under a name the manifest may call and dispatched to a caller's closure.
//!
//! Three properties make overlays safe to reason about. An overlay may replace
//! a helper that already exists — the replaced function is handed to the
//! dispatch as [`OverlayCall::shadowed`], so a test can wrap rather than
//! discard it — but it may not take a name the loader or the manifest has
//! already claimed; that is [`TemplateOverlayError::NameCollision`]. Overlays
//! apply to test loads only, so build scaffolding cannot be reinterpreted by a
//! test fixture. And the mechanism is compiled unconditionally: overlays are a
//! loader capability rather than a test-only code path (C9).

use std::fmt;
use std::sync::Arc;

use indexmap::IndexMap;
use minijinja::value::{Kwargs, Rest, Value};
use minijinja::{Environment, Error, State};

use super::super::ManifestValue;
use super::super::jinja_macros::parse_macro_name;
use crate::ast::MacroDefinition;
use crate::localization::{self, keys};

/// The name the loader reserves for its environment seam.
const ENVIRONMENT_SEAM_NAME: &str = "env";

/// The prefix of the globals only the loader may define.
const INTERNAL_NAME_PREFIX: &str = "__netsuke_";

/// Template functions a test load registers in place of withheld helpers.
///
/// Overlays are applied at the end of registration, so they see — and may
/// replace — every helper the stdlib and the loader installed before them.
#[derive(Clone, Default)]
pub struct TemplateOverlays {
    /// Callables in registration order, which fixes which collision a load
    /// reports when a test names two claimed names.
    functions: IndexMap<String, OverlayCallable>,
}

impl TemplateOverlays {
    /// Add a template function named `name`.
    ///
    /// The name is checked against the manifest's own namespace when the
    /// manifest is loaded, because only a load knows which `vars` keys and
    /// macros the manifest defines.
    #[must_use]
    pub fn with_function(mut self, name: impl Into<String>, callable: OverlayCallable) -> Self {
        self.functions.insert(name.into(), callable);
        self
    }

    /// Report whether the test supplied no overlays at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.functions.is_empty()
    }

    /// Iterate the overlays in registration order.
    fn entries(&self) -> impl Iterator<Item = (&str, &OverlayCallable)> {
        self.functions
            .iter()
            .map(|(name, callable)| (name.as_str(), callable))
    }
}

/// The type-erased dispatch a template overlay runs.
///
/// Named rather than written inline: the same signature appears in the stored
/// callable, its constructor bound, and the registered closure, and a single
/// alias keeps those three in step.
type Dispatch = dyn Fn(&OverlayCall<'_, '_>) -> Result<Value, Error> + Send + Sync;

/// A caller-supplied implementation behind one template overlay.
///
/// `MiniJinja` registers a plain closure, so the dispatch is type-erased here:
/// a test supplies whatever closure its fixtures need, and the loader keeps one
/// uniform value.
#[derive(Clone)]
pub struct OverlayCallable(
    /// The dispatch invoked for each call the template makes.
    Arc<Dispatch>,
);

impl OverlayCallable {
    /// Wrap `dispatch` as a template overlay.
    ///
    /// # Examples
    ///
    /// ```
    /// use minijinja::value::Value;
    /// use netsuke::manifest::{OverlayCall, OverlayCallable, TemplateOverlays};
    ///
    /// let overlays = TemplateOverlays::default().with_function(
    ///     "answer",
    ///     OverlayCallable::new(|_call: &OverlayCall<'_, '_>| Ok(Value::from(42))),
    /// );
    /// assert!(!overlays.is_empty());
    /// ```
    #[must_use]
    pub fn new<F>(dispatch: F) -> Self
    where
        F: Fn(&OverlayCall<'_, '_>) -> Result<Value, Error> + Send + Sync + 'static,
    {
        Self(Arc::new(dispatch))
    }

    /// Run the dispatch for one call.
    fn invoke(&self, call: &OverlayCall<'_, '_>) -> Result<Value, Error> {
        (self.0)(call)
    }
}

/// One call a template made to an overlay.
///
/// The borrows come from the environment that is rendering, so an overlay can
/// forward a call to the function it replaced without copying anything.
pub struct OverlayCall<'call, 'env> {
    /// Rendering state, which a replaced function may also need.
    state: &'call State<'call, 'env>,
    /// Positional arguments, with keyword arguments appended as one trailing
    /// [`Value`] when the template passed any.
    args: &'call [Value],
    /// The function this overlay replaced, if the name was already registered.
    shadowed: Option<&'call Value>,
}

impl<'call, 'env> OverlayCall<'call, 'env> {
    /// Return the rendering state the template called through.
    #[must_use]
    pub const fn state(&self) -> &'call State<'call, 'env> {
        self.state
    }

    /// Return the call's arguments.
    ///
    /// Keyword arguments arrive as a single trailing kwargs map, so this slice
    /// is exactly the argument list a replaced function accepts.
    #[must_use]
    pub const fn args(&self) -> &'call [Value] {
        self.args
    }

    /// Return the function this overlay replaced, if there was one.
    ///
    /// A name no helper had yet yields `None`. A replaced helper is handed back
    /// as a value, so `shadowed().call(state(), args())` reproduces the call
    /// the overlay displaced.
    #[must_use]
    pub const fn shadowed(&self) -> Option<&'call Value> {
        self.shadowed
    }
}

/// Reasons a test load rejected its template overlays.
#[derive(Debug)]
pub enum TemplateOverlayError {
    /// An overlay name is one the load already gives another meaning.
    NameCollision {
        /// The overlay name that collided.
        name: String,
        /// The namespace the name already belongs to.
        kind: OverlayNameKind,
    },
}

impl fmt::Display for TemplateOverlayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NameCollision { name, kind } => {
                let text = localization::message(keys::MANIFEST_OVERLAY_NAME_COLLISION)
                    .with_arg("name", name)
                    .with_arg("kind", kind.to_string())
                    .to_string();
                formatter.write_str(&text)
            }
        }
    }
}

impl std::error::Error for TemplateOverlayError {}

/// The namespace an overlay name collided with.
///
/// The rendered form is the token the collision message interpolates, so it
/// stays stable and untranslated: it names the namespace, not the prose around
/// it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OverlayNameKind {
    /// The loader's `env()` seam.
    EnvironmentSeam,
    /// A global the loader owns by prefix.
    Internal,
    /// A key in the manifest's `vars` section.
    ManifestVariable,
    /// A macro the manifest defines.
    ManifestMacro,
}

impl fmt::Display for OverlayNameKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::EnvironmentSeam => "environment-seam",
            Self::Internal => "loader-internal",
            Self::ManifestVariable => "manifest-variable",
            Self::ManifestMacro => "manifest-macro",
        })
    }
}

/// Names the loader and the manifest already claim in the template namespace.
///
/// The manifest half is computed once, before its `vars` are registered, and is
/// reused by the reserved-name check that registration performs, so the two
/// checks cannot disagree about which names a manifest claims.
pub(crate) struct OverlayNamespace {
    /// Keys declared under the manifest's `vars` section.
    variables: Vec<String>,
    /// Macro names the manifest defines, in declaration order.
    macros: Vec<String>,
}

impl OverlayNamespace {
    /// Return the first `vars` key the loader reserves, if one is declared.
    pub(crate) fn reserved_variable(&self, reserved: &[&str]) -> Option<&str> {
        self.variables
            .iter()
            .map(String::as_str)
            .find(|key| reserved.contains(key))
    }

    /// Classify `name` against the names already claimed.
    ///
    /// `glob()` is deliberately absent: the loader registers it, but replacing
    /// it is the reason overlays exist.
    fn collision(&self, name: &str) -> Option<OverlayNameKind> {
        if name == ENVIRONMENT_SEAM_NAME {
            Some(OverlayNameKind::EnvironmentSeam)
        } else if name.starts_with(INTERNAL_NAME_PREFIX) {
            Some(OverlayNameKind::Internal)
        } else if self.variables.iter().any(|key| key == name) {
            Some(OverlayNameKind::ManifestVariable)
        } else if self.macros.iter().any(|macro_name| macro_name == name) {
            Some(OverlayNameKind::ManifestMacro)
        } else {
            None
        }
    }
}

/// Read the names a manifest has already claimed in the template namespace.
///
/// A malformed `macros` section yields no macro names rather than an error: the
/// macro registration reports that diagnosis in its own step, and this function
/// must not pre-empt it with a different message.
pub(crate) fn overlay_namespace(doc: &ManifestValue) -> OverlayNamespace {
    OverlayNamespace {
        variables: declared_variables(doc),
        macros: declared_macros(doc),
    }
}

/// Return the keys of the manifest's `vars` section.
fn declared_variables(doc: &ManifestValue) -> Vec<String> {
    doc.get("vars")
        .and_then(ManifestValue::as_object)
        .map_or_default(|vars| vars.keys().cloned().collect())
}

/// Return the names of the macros the manifest defines.
fn declared_macros(doc: &ManifestValue) -> Vec<String> {
    let Some(macros) = doc.get("macros") else {
        return Vec::new();
    };
    serde_json::from_value::<Vec<MacroDefinition>>(macros.clone())
        .unwrap_or_default()
        .iter()
        .filter_map(|definition| parse_macro_name(&definition.signature).ok())
        .collect()
}

/// Reject overlays that would take a name the manifest or loader already uses.
fn validate_overlay_names(
    overlays: &TemplateOverlays,
    namespace: &OverlayNamespace,
) -> Result<(), TemplateOverlayError> {
    overlays
        .entries()
        .find_map(|(name, _callable)| {
            namespace
                .collision(name)
                .map(|kind| TemplateOverlayError::NameCollision {
                    name: name.to_owned(),
                    kind,
                })
        })
        .map_or(Ok(()), Err)
}

/// Register a test load's overlays after every other template helper.
///
/// Registering last is what lets an overlay replace a helper the stdlib or the
/// loader already installed. Validating first means a rejected overlay leaves
/// the environment untouched.
///
/// # Errors
///
/// Returns [`TemplateOverlayError::NameCollision`] when an overlay name is one
/// the manifest or the loader has already claimed.
pub(crate) fn register_overlays(
    jinja: &mut Environment<'static>,
    overlays: Option<&TemplateOverlays>,
    namespace: &OverlayNamespace,
) -> Result<(), TemplateOverlayError> {
    let Some(supplied) = overlays else {
        return Ok(());
    };
    validate_overlay_names(supplied, namespace)?;
    for (name, callable) in supplied.entries() {
        register_overlay(jinja, name.to_owned(), callable.clone());
    }
    Ok(())
}

/// Register one overlay, capturing the function it replaces.
fn register_overlay(jinja: &mut Environment<'static>, name: String, callable: OverlayCallable) {
    let shadowed = find_registered(jinja, &name);
    jinja.add_function(
        name,
        move |state: &State, args: Rest<Value>, kwargs: Kwargs| {
            let positional = with_trailing_kwargs(args.0, &kwargs);
            callable.invoke(&OverlayCall {
                state,
                args: &positional,
                shadowed: shadowed.as_ref(),
            })
        },
    );
}

/// Return the function already registered under `name`, if any.
///
/// `MiniJinja` has no lookup by name, but globals enumerate and functions are
/// stored as globals, so the replaced function is still reachable and can be
/// handed to the overlay instead of being lost.
fn find_registered(jinja: &Environment<'static>, name: &str) -> Option<Value> {
    jinja
        .globals()
        .find(|(existing, _value)| *existing == name)
        .map(|(_existing, value)| value)
}

/// Append the call's keyword arguments as one trailing value.
///
/// `MiniJinja` hands a function its keyword arguments separately, while a value
/// made from them is extracted back as keyword arguments by whatever function
/// receives it, so appending one reproduces the original call.
fn with_trailing_kwargs(mut positional: Vec<Value>, kwargs: &Kwargs) -> Vec<Value> {
    if kwargs.args().next().is_some() {
        positional.push(Value::from(kwargs.clone()));
    }
    positional
}
