//! Admit only manifest schema releases the reader can interpret.
//!
//! This module owns the pure schema-version predicate and the extraction of
//! the literal version field from the initially parsed YAML value. It does not
//! read the environment or filesystem, and non-mapping roots remain available
//! to the existing structural deserializer.
//!
//! [`telemetry`] observes the same single gate. The predicate decides and
//! stays free of a metrics recorder; the telemetry child records the decision
//! at the one boundary every loaded document crosses, so the counter's series
//! count is fixed here rather than by anything a manifest supplies.

use semver::Version;
use serde_json::Value;

mod telemetry;

pub(in crate::manifest) use telemetry::record_admission;
pub use telemetry::{SCHEMA_ADMISSION_OUTCOME_VALUES, SCHEMA_ADMISSION_TOTAL};

/// Schema family and greatest minor understood by this reader.
pub(super) struct SupportedSchema {
    /// Major schema version accepted by this reader.
    pub(super) major: u64,
    /// Greatest admitted minor within [`Self::major`].
    pub(super) max_minor: u64,
}

/// Supported schema release boundary for the current reader.
pub(super) const SUPPORTED: SupportedSchema = SupportedSchema {
    major: 1,
    max_minor: 0,
};

/// A safe, typed reason why a manifest schema declaration cannot be admitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SchemaVersionRejection {
    /// A mapping-rooted manifest omitted `netsuke_version`.
    Missing,
    /// The `netsuke_version` value is not a YAML string.
    NotAString,
    /// The string is not valid semantic-version syntax.
    Invalid,
    /// The valid semantic version is outside the reader's supported releases.
    Unsupported {
        /// Major component, safe to render without retaining user text.
        major: u64,
        /// Minor component, safe to render without retaining user text.
        minor: u64,
    },
    /// The version falls inside the supported range but is not exactly
    /// `MAJOR.MINOR.0`, so it is not a schema release at all.
    NotCanonical,
}

/// Check whether `version` is an exact release understood by this reader.
///
/// # Examples
///
/// ```ignore
/// let supported = Version::parse("1.0.0").expect("valid semantic version");
/// let reserved = Version::parse("1.1.0").expect("valid semantic version");
/// assert!(admit(&supported).is_ok());
/// assert!(matches!(admit(&reserved), Err(SchemaVersionRejection::Unsupported { .. })));
/// ```
///
/// # Errors
///
/// Returns [`SchemaVersionRejection::Unsupported`] when the major or minor is
/// outside the reader's supported releases, and
/// [`SchemaVersionRejection::NotCanonical`] when the version is inside that
/// range but carries a non-zero patch, a prerelease, or build metadata.
///
/// The two rejections are distinct on purpose. A version inside the range is
/// not an "unsupported schema" the reader could ever admit by upgrading, so
/// reporting it that way would name the declared and supported schema as the
/// same value and hide the canonical-form rule that actually failed.
pub(super) fn admit(version: &Version) -> Result<(), SchemaVersionRejection> {
    if !within_supported_range(version) {
        return Err(SchemaVersionRejection::Unsupported {
            major: version.major,
            minor: version.minor,
        });
    }
    if !is_canonical_release(version) {
        return Err(SchemaVersionRejection::NotCanonical);
    }
    Ok(())
}

/// Check whether the major and minor name a release this reader supports.
///
/// The patch, prerelease, and build components are deliberately ignored here;
/// [`is_canonical_release`] judges those, so a supported family with a
/// malformed release reports [`SchemaVersionRejection::NotCanonical`] rather
/// than the self-contradictory "declares 1.0, supports 1.0" rejection.
fn within_supported_range(version: &Version) -> bool {
    version.major == SUPPORTED.major && (0..=SUPPORTED.max_minor).contains(&version.minor)
}

/// Check whether `version` is exactly `MAJOR.MINOR.0` with no extras.
fn is_canonical_release(version: &Version) -> bool {
    version.patch == 0 && version.pre.is_empty() && version.build.is_empty()
}

/// Admit a parsed document's literal schema declaration when its root is a map.
///
/// # Examples
///
/// ```ignore
/// let document = serde_saphyr::from_str::<Value>(
///     "netsuke_version: '1.0.0'\n",
/// )
/// .expect("valid YAML");
/// assert_eq!(admit_document(&document), Ok(()));
/// ```
///
/// # Errors
///
/// Returns a typed rejection when a mapping root omits the version, declares a
/// non-string or malformed value, or names a release this reader cannot load.
pub(super) fn admit_document(document: &Value) -> Result<(), SchemaVersionRejection> {
    if document.as_object().is_none() {
        return Ok(());
    }
    let version_value = document
        .get("netsuke_version")
        .ok_or(SchemaVersionRejection::Missing)?;
    let raw_version = version_value
        .as_str()
        .ok_or(SchemaVersionRejection::NotAString)?;
    let version = Version::parse(raw_version).map_err(|_| SchemaVersionRejection::Invalid)?;
    admit(&version)
}

#[cfg(test)]
mod tests;
