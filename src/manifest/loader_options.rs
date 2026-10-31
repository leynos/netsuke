//! Options that decide how one manifest load is performed.
//!
//! A load's host-facing capabilities are decided once, by
//! [`StdlibRegistration`], rather than separately by each entry point: the same
//! decision selects the stdlib surface, decides whether `env()` and `glob()`
//! are real or refusing, picks the render mode, and gates expansion telemetry
//! and the query-refusal classifier (C5). The public surface is
//! [`ManifestLoadOptions`]; the registration enum and its dispositions stay
//! crate-private so the roadmap can extend the sandbox without freezing them.
//!
//! There is deliberately no public `Default`: a default silently selecting the
//! ambient stdlib and process environment would be a trap for an embedder.

use std::sync::OnceLock;

use camino::Utf8PathBuf;
use minijinja::Environment;

use super::{
    EnvAccessPolicy, EnvReader, ManifestBudgetLimits, ManifestEnvironment, ManifestLoadStage,
    ManifestName, ManifestParse, NetsukeManifest,
    budget::adapter::from_str_named,
    registration::{register_env_function, register_glob_function},
};
use crate::stdlib::{StdlibConfig, StdlibState};

/// State whether the loader installs a real host-reading helper or a refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HostHelperDisposition {
    /// The loader installs the real helper.
    Real,
    /// The loader installs the restricted surface's refusal stub.
    Refused,
}

/// Selects the stdlib surface available while rendering a manifest.
///
/// Every capability decision for a load is answered from this one value, and
/// no second mode enum survives (C5).
pub(crate) enum StdlibRegistration {
    /// The complete stdlib, configured from the process's ambient state.
    AmbientFull,
    /// The complete stdlib, configured by the caller.
    Full(Box<StdlibConfig>),
    /// The read-only stdlib used to inspect manifest discovery metadata.
    ManifestQuery,
}

impl StdlibRegistration {
    /// Decide whether `env()` reads the injected environment or refuses.
    pub(crate) const fn env_disposition(&self) -> HostHelperDisposition {
        match self {
            Self::AmbientFull | Self::Full(_) => HostHelperDisposition::Real,
            Self::ManifestQuery => HostHelperDisposition::Refused,
        }
    }

    /// Decide whether `glob()` inspects the filesystem or refuses.
    pub(crate) const fn glob_disposition(&self) -> HostHelperDisposition {
        match self {
            Self::AmbientFull | Self::Full(_) => HostHelperDisposition::Real,
            Self::ManifestQuery => HostHelperDisposition::Refused,
        }
    }

    /// Report whether rendering uses the manifest-query render mode.
    pub(crate) const fn uses_query_rendering(&self) -> bool {
        matches!(self, Self::ManifestQuery)
    }

    /// Report whether expansion and budget telemetry is recorded for this load.
    ///
    /// Those counters describe normal manifest loading; counting discovery
    /// queries or test runs would distort usage evidence (ADR-0050 Addendum E).
    pub(crate) const fn records_expansion_and_budget_telemetry(&self) -> bool {
        matches!(self, Self::AmbientFull | Self::Full(_))
    }

    /// Report whether a refused helper marks the surrounding `when` as
    /// conditional instead of failing the load.
    pub(crate) const fn classifies_query_refusals(&self) -> bool {
        matches!(self, Self::ManifestQuery)
    }

    /// Report whether this mode may carry template overlays.
    ///
    /// Overlays are a capability grant, so only the test surface admits them.
    /// No variant grants it yet, and the overlay machinery lands with the test
    /// surface (EP-M4); the decision table asserts the refusal now so a new
    /// variant cannot be added without stating its answer.
    #[cfg(test)]
    pub(crate) const fn accepts_overlays(&self) -> bool {
        match self {
            Self::AmbientFull | Self::Full(_) | Self::ManifestQuery => false,
        }
    }
}

/// Inputs to one manifest load beyond the YAML text and its name.
pub struct ManifestLoadOptions<'a> {
    /// Decides every capability for this load.
    registration: StdlibRegistration,
    /// Reader and access policy behind `env()`.
    environment: ManifestEnvironment<'a>,
    /// Resource ceilings resolved before loading.
    budget_limits: ManifestBudgetLimits,
    /// Observes load stages without changing parsing behaviour.
    on_stage: Option<&'a mut dyn FnMut(ManifestLoadStage)>,
}

impl<'a> ManifestLoadOptions<'a> {
    /// Load with the complete build stdlib and explicit environment inputs.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::manifest::{
    ///     EnvAccessPolicy, ManifestEnvironment, ManifestLoadOptions, process_env_reader,
    /// };
    /// use netsuke::stdlib::StdlibConfig;
    ///
    /// let reader = process_env_reader();
    /// let environment = ManifestEnvironment::new(&reader, EnvAccessPolicy::default());
    /// let options = ManifestLoadOptions::full(
    ///     StdlibConfig::from_current_dir().expect("open the workspace"),
    ///     environment,
    /// );
    /// # let _ = options;
    /// ```
    #[must_use]
    pub fn full(config: StdlibConfig, environment: ManifestEnvironment<'a>) -> Self {
        Self {
            registration: StdlibRegistration::Full(Box::new(config)),
            environment,
            budget_limits: ManifestBudgetLimits::default(),
            on_stage: None,
        }
    }

    /// Load with the ambient build stdlib and explicit environment inputs.
    ///
    /// This is the mode behind [`super::from_str`]: the stdlib is rooted at the
    /// process working directory, exactly as before options existed.
    #[must_use]
    pub(crate) fn ambient(environment: ManifestEnvironment<'a>) -> Self {
        Self {
            registration: StdlibRegistration::AmbientFull,
            environment,
            budget_limits: ManifestBudgetLimits::default(),
            on_stage: None,
        }
    }

    /// Load for a side-effect-free discovery query.
    ///
    /// Query loads never read the environment, so they supply a reader that
    /// refuses every lookup rather than accepting one that could contradict the
    /// refusal stubs the registration installs.
    #[must_use]
    pub(crate) fn manifest_query<'options>() -> ManifestLoadOptions<'options> {
        ManifestLoadOptions {
            registration: StdlibRegistration::ManifestQuery,
            environment: query_environment(),
            budget_limits: ManifestBudgetLimits::default(),
            on_stage: None,
        }
    }

    /// Placeholder for a path load whose registration the workspace decides.
    ///
    /// The registration here is never used: [`super::query::from_path_with_options`]
    /// always replaces it with the one its `registration_for` closure derives
    /// from the opened workspace. Keeping the field populated lets the same
    /// builder chain serve string and path loads.
    #[must_use]
    pub(crate) fn deferred(environment: ManifestEnvironment<'a>) -> Self {
        Self {
            registration: StdlibRegistration::AmbientFull,
            environment,
            budget_limits: ManifestBudgetLimits::default(),
            on_stage: None,
        }
    }

    /// Supply resource ceilings.
    #[must_use]
    pub const fn with_budget_limits(mut self, limits: ManifestBudgetLimits) -> Self {
        self.budget_limits = limits;
        self
    }

    /// Observe loader stages.
    #[must_use]
    pub fn with_stage_observer(mut self, on_stage: &'a mut dyn FnMut(ManifestLoadStage)) -> Self {
        self.on_stage = Some(on_stage);
        self
    }

    /// Replace the registration with one derived from a workspace.
    #[must_use]
    pub(super) fn with_registration(mut self, registration: StdlibRegistration) -> Self {
        self.registration = registration;
        self
    }

    /// Report a load stage to the observer, if one is installed.
    pub(super) fn notify(&mut self, stage: ManifestLoadStage) {
        super::loading::notify_stage(&mut self.on_stage, stage);
    }

    /// Resolve these options into the loader's internal parse request.
    pub(super) fn into_parse<'name>(
        self,
        name: &'name ManifestName,
        manifest_root: Option<Utf8PathBuf>,
    ) -> ManifestParse<'a, 'name> {
        ManifestParse {
            name,
            manifest_root,
            registration: self.registration,
            environment: self.environment,
            budget_limits: self.budget_limits,
            on_stage: self.on_stage,
        }
    }
}

/// Load a manifest string with explicit options.
///
/// # Examples
///
/// ```
/// use netsuke::manifest::{
///     EnvAccessPolicy, ManifestEnvironment, ManifestLoadOptions, ManifestName,
///     from_str_with_options, process_env_reader,
/// };
/// use netsuke::stdlib::StdlibConfig;
///
/// let reader = process_env_reader();
/// let environment = ManifestEnvironment::new(&reader, EnvAccessPolicy::default());
/// let options = ManifestLoadOptions::full(
///     StdlibConfig::from_current_dir().expect("open the workspace"),
///     environment,
/// );
/// let yaml = concat!(
///     "netsuke_version: 1.0.0\n",
///     "targets:\n",
///     "  - name: build\n",
///     "    command: echo hi\n",
/// );
/// let manifest = from_str_with_options(yaml, &ManifestName::new("Netsukefile"), options)
///     .expect("load manifest");
/// assert_eq!(manifest.targets.len(), 1);
/// ```
///
/// # Errors
///
/// Returns an error if YAML parsing, template evaluation, or recipe validation
/// fails, or if the load exhausts its resource budget.
pub fn from_str_with_options<'a>(
    yaml: &str,
    name: &'a ManifestName,
    options: ManifestLoadOptions<'a>,
) -> anyhow::Result<NetsukeManifest> {
    from_str_named(yaml, options.into_parse(name, None))
}

/// Register `env()` and `glob()` from the load's registration decision.
///
/// The loader owns these two helpers rather than the stdlib registrations,
/// because whether a load may read the host is a property of the load, not of
/// the library surface (C5). The refusal shapes still come from the restricted
/// surface's inventory, so query refusal text is unchanged (C2).
pub(super) fn register_host_helpers(
    env: &mut Environment<'_>,
    registration: &StdlibRegistration,
    environment: &ManifestEnvironment<'_>,
    manifest_root: Option<Utf8PathBuf>,
) {
    match registration.env_disposition() {
        HostHelperDisposition::Real => {
            register_env_function(env, environment.reader(), environment.access_policy());
        }
        HostHelperDisposition::Refused => crate::stdlib::register_env_refusal(env),
    }
    match registration.glob_disposition() {
        HostHelperDisposition::Real => register_glob_function(env, manifest_root),
        HostHelperDisposition::Refused => crate::stdlib::register_glob_refusal(env),
    }
}

/// Register the stdlib surface the registration selects.
pub(super) fn register_stdlib(
    env: &mut Environment<'_>,
    registration: StdlibRegistration,
) -> anyhow::Result<StdlibState> {
    match registration {
        StdlibRegistration::AmbientFull => crate::stdlib::register(env),
        StdlibRegistration::Full(config) => crate::stdlib::register_with_config(env, *config),
        StdlibRegistration::ManifestQuery => Ok(crate::stdlib::register_manifest_query(env)),
    }
}

/// Return a `'static` environment whose reader refuses every lookup.
///
/// Query options live for as long as their caller needs them, so the reader
/// they carry must be static; one shared instance is enough because it holds no
/// state. The refusal stubs mean this reader is never actually consulted.
fn query_environment() -> ManifestEnvironment<'static> {
    static READER: OnceLock<EnvReader> = OnceLock::new();
    let reader = READER.get_or_init(super::env::disabled_env_reader);
    ManifestEnvironment::new(reader, EnvAccessPolicy::default())
}
