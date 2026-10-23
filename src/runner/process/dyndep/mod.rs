//! Process-level dyndep sidecar publication, retention, and telemetry.

mod files;
mod retention;
mod telemetry;

pub(crate) use files::materialize_dyndep_files;
pub use retention::MAX_RETAINED_DYNDEP_FILES;
pub(crate) use retention::{DyndepPublicationLease, prune_dyndep_cache};
