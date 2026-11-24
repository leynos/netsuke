//! Register the host helpers and stdlib surface a load's registration selects.
//!
//! Split from [`super`] to keep each module within the repository's line cap.
//! This half owns the effects of a registration decision: installing `env()`
//! and `glob()` as real helpers or refusals, and selecting the stdlib surface.
//! The decision type itself stays in the parent.

use camino::Utf8PathBuf;
use minijinja::Environment;

use super::super::{
    CollectionAllowance, ManifestEnvironment, StdlibRegistration,
    registration::{register_env_function, register_glob_function},
};
use super::HostHelperDisposition;
use crate::stdlib::StdlibState;

/// Register `env()` and `glob()` from the load's registration decision.
///
/// The loader owns these two helpers rather than the stdlib registrations,
/// because whether a load may read the host is a property of the load, not of
/// the library surface (C5). The refusal shapes still come from the restricted
/// surface's inventory, so query refusal text is unchanged (C2).
pub(crate) fn register_host_helpers(
    env: &mut Environment<'_>,
    registration: &StdlibRegistration,
    environment: &ManifestEnvironment<'_>,
    manifest_root: Option<Utf8PathBuf>,
) {
    match registration.env_disposition() {
        HostHelperDisposition::Real => {
            let reader = registration.env_reader(environment.reader());
            register_env_function(env, reader, environment.access_policy());
        }
        HostHelperDisposition::Refused(_) => crate::stdlib::register_env_refusal(env),
    }
    match registration.glob_disposition() {
        HostHelperDisposition::Real => register_glob_function(env, manifest_root),
        HostHelperDisposition::Refused(surface) => {
            crate::stdlib::register_glob_refusal(env, surface);
        }
    }
}

/// Register the stdlib surface the registration selects.
///
/// The collection allowance comes from the load's manifest budget, so every
/// surface that can evaluate a template charges the same shared counters.
pub(crate) fn register_stdlib(
    env: &mut Environment<'_>,
    registration: StdlibRegistration,
    collection_allowance: &CollectionAllowance,
) -> anyhow::Result<StdlibState> {
    match registration {
        StdlibRegistration::AmbientFull => {
            crate::stdlib::register_with_collection_allowance(env, collection_allowance)
        }
        StdlibRegistration::Full(config) => Ok(
            crate::stdlib::register_with_config_and_collection_allowance(
                env,
                *config,
                collection_allowance,
            ),
        ),
        StdlibRegistration::ManifestQuery => Ok(crate::stdlib::register_manifest_query(
            env,
            collection_allowance,
        )),
        StdlibRegistration::Test(surface) => Ok(crate::stdlib::register_for_test(
            env,
            surface.config(),
            collection_allowance,
        )),
    }
}
