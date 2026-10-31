//! Inventory and refuse template helpers that cross a restricted load boundary.
//!
//! Manifest query and test loads expose only explicitly permitted helpers.
//! Keeping their dispositions here makes the inventory the shared source for
//! refusals and for the build registration's file-test names.

/// Classify the `MiniJinja` registration shape of a template helper.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HelperKind {
    /// A callable written as `name(...)`.
    Function,
    /// A value transformation written as `value | name`.
    Filter,
    /// A predicate written as `value is name`.
    Test,
}

/// State whether a helper may be used during manifest-query rendering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum QueryDisposition {
    /// The helper is safe and available during manifest queries.
    Available,
    /// The helper must produce the established query refusal.
    Refused,
}

/// State how a helper is exposed during test-mode rendering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TestDisposition {
    /// The helper is deterministic and available during test loads.
    Available,
    /// The helper is unavailable until a later sandbox adapter is added.
    Refused,
    /// The helper is available only when the caller injects a clock.
    ClockInjected,
}

/// Describe one registered helper and its restricted-load dispositions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct HelperEntry {
    /// Name exposed to `MiniJinja`.
    name: &'static str,
    /// Registration shape exposed to `MiniJinja`.
    kind: HelperKind,
    /// Manifest-query availability.
    query: QueryDisposition,
    /// Test-mode availability.
    test: TestDisposition,
}

/// Select the unavailable-helper inventory for a restricted load surface.
#[derive(Clone, Copy)]
enum RestrictedSurface {
    /// The side-effect-free surface used by manifest discovery.
    ManifestQuery,
}

impl RestrictedSurface {
    /// Report whether this surface refuses one inventory entry.
    fn refuses(self, entry: &HelperEntry) -> bool {
        match self {
            Self::ManifestQuery => entry.query == QueryDisposition::Refused,
        }
    }
}

/// Construct one inventory row without repeating the field labels.
const fn helper(
    name: &'static str,
    kind: HelperKind,
    query: QueryDisposition,
    test: TestDisposition,
) -> HelperEntry {
    HelperEntry {
        name,
        kind,
        query,
        test,
    }
}

/// Inventory template registrations whose behaviour differs by load surface.
///
/// Keep both query and test dispositions beside each registration. Query
/// refusal stubs and the real file-test registration consume this list now;
/// test-mode registration and its completeness contract consume it in EP-M3.
const SURFACE: &[HelperEntry] = &[
    helper(
        "env",
        HelperKind::Function,
        QueryDisposition::Refused,
        TestDisposition::Available,
    ),
    helper(
        "glob",
        HelperKind::Function,
        QueryDisposition::Refused,
        TestDisposition::Refused,
    ),
    helper(
        "fetch",
        HelperKind::Function,
        QueryDisposition::Refused,
        TestDisposition::Refused,
    ),
    helper(
        "which",
        HelperKind::Filter,
        QueryDisposition::Refused,
        TestDisposition::Refused,
    ),
    helper(
        "which",
        HelperKind::Function,
        QueryDisposition::Refused,
        TestDisposition::Refused,
    ),
    helper(
        "command_available",
        HelperKind::Function,
        QueryDisposition::Refused,
        TestDisposition::Refused,
    ),
    helper(
        "now",
        HelperKind::Function,
        QueryDisposition::Refused,
        TestDisposition::ClockInjected,
    ),
    helper(
        "timedelta",
        HelperKind::Function,
        QueryDisposition::Available,
        TestDisposition::Available,
    ),
    helper(
        "shell",
        HelperKind::Filter,
        QueryDisposition::Refused,
        TestDisposition::Refused,
    ),
    helper(
        "grep",
        HelperKind::Filter,
        QueryDisposition::Refused,
        TestDisposition::Refused,
    ),
    helper(
        "contents",
        HelperKind::Filter,
        QueryDisposition::Refused,
        TestDisposition::Refused,
    ),
    helper(
        "realpath",
        HelperKind::Filter,
        QueryDisposition::Refused,
        TestDisposition::Refused,
    ),
    helper(
        "expanduser",
        HelperKind::Filter,
        QueryDisposition::Refused,
        TestDisposition::Refused,
    ),
    helper(
        "size",
        HelperKind::Filter,
        QueryDisposition::Refused,
        TestDisposition::Refused,
    ),
    helper(
        "linecount",
        HelperKind::Filter,
        QueryDisposition::Refused,
        TestDisposition::Refused,
    ),
    helper(
        "hash",
        HelperKind::Filter,
        QueryDisposition::Refused,
        TestDisposition::Refused,
    ),
    helper(
        "digest",
        HelperKind::Filter,
        QueryDisposition::Refused,
        TestDisposition::Refused,
    ),
    helper(
        "basename",
        HelperKind::Filter,
        QueryDisposition::Available,
        TestDisposition::Available,
    ),
    helper(
        "dirname",
        HelperKind::Filter,
        QueryDisposition::Available,
        TestDisposition::Available,
    ),
    helper(
        "with_suffix",
        HelperKind::Filter,
        QueryDisposition::Available,
        TestDisposition::Available,
    ),
    helper(
        "relative_to",
        HelperKind::Filter,
        QueryDisposition::Available,
        TestDisposition::Available,
    ),
    helper(
        "shell_quote",
        HelperKind::Filter,
        QueryDisposition::Available,
        TestDisposition::Available,
    ),
    helper(
        "shell_join",
        HelperKind::Filter,
        QueryDisposition::Available,
        TestDisposition::Available,
    ),
    helper(
        "uniq",
        HelperKind::Filter,
        QueryDisposition::Available,
        TestDisposition::Available,
    ),
    helper(
        "flatten",
        HelperKind::Filter,
        QueryDisposition::Available,
        TestDisposition::Available,
    ),
    helper(
        "compact",
        HelperKind::Filter,
        QueryDisposition::Available,
        TestDisposition::Available,
    ),
    helper(
        "group_by",
        HelperKind::Filter,
        QueryDisposition::Available,
        TestDisposition::Available,
    ),
    helper(
        "dir",
        HelperKind::Test,
        QueryDisposition::Refused,
        TestDisposition::Refused,
    ),
    helper(
        "file",
        HelperKind::Test,
        QueryDisposition::Refused,
        TestDisposition::Refused,
    ),
    helper(
        "symlink",
        HelperKind::Test,
        QueryDisposition::Refused,
        TestDisposition::Refused,
    ),
    helper(
        "pipe",
        HelperKind::Test,
        QueryDisposition::Refused,
        TestDisposition::Refused,
    ),
    helper(
        "block_device",
        HelperKind::Test,
        QueryDisposition::Refused,
        TestDisposition::Refused,
    ),
    helper(
        "char_device",
        HelperKind::Test,
        QueryDisposition::Refused,
        TestDisposition::Refused,
    ),
    helper(
        "device",
        HelperKind::Test,
        QueryDisposition::Refused,
        TestDisposition::Refused,
    ),
];

mod query;
pub(crate) use query::{
    is_manifest_query_disabled_error, register_env_refusal, register_glob_refusal,
};

/// Register the deliberate failures selected for manifest-query loads.
pub(super) fn register_query_refusals(env: &mut minijinja::Environment<'_>) {
    query::register_refusals(env, RestrictedSurface::ManifestQuery);
}

/// Return the names used by the build surface's real file-test registrations.
pub(super) fn file_test_names() -> impl Iterator<Item = &'static str> {
    SURFACE
        .iter()
        .filter(|entry| entry.kind == HelperKind::Test)
        .map(|entry| entry.name)
}

#[cfg(test)]
mod tests;
