//! Retain monotonic manifest-budget requests across project configuration layers.

/// Manifest-budget restrictions requested by the project configuration chain.
#[derive(Debug, Default)]
pub(crate) struct ProjectManifestBudgetRequest {
    /// Requested per-evaluation instruction limit.
    pub(crate) evaluation_fuel: Option<u64>,
    /// Requested aggregate instruction limit.
    pub(crate) manifest_fuel: Option<u64>,
    /// Requested per-value rendered-byte limit.
    pub(crate) rendered_value_bytes: Option<usize>,
    /// Requested aggregate rendered-byte limit.
    pub(crate) rendered_manifest_bytes: Option<usize>,
    /// Requested aggregate source-byte limit.
    pub(crate) source_bytes: Option<usize>,
    /// Requested per-foreach cardinality limit.
    pub(crate) foreach_cardinality: Option<usize>,
    /// Requested aggregate expansion count.
    pub(crate) expanded_entries: Option<usize>,
}

impl ProjectManifestBudgetRequest {
    /// Retain the most restrictive value requested by project-controlled layers.
    pub(crate) fn narrow_with(&mut self, other: &Self) {
        narrow_limit(&mut self.evaluation_fuel, other.evaluation_fuel);
        narrow_limit(&mut self.manifest_fuel, other.manifest_fuel);
        narrow_limit(&mut self.rendered_value_bytes, other.rendered_value_bytes);
        narrow_limit(
            &mut self.rendered_manifest_bytes,
            other.rendered_manifest_bytes,
        );
        narrow_limit(&mut self.source_bytes, other.source_bytes);
        narrow_limit(&mut self.foreach_cardinality, other.foreach_cardinality);
        narrow_limit(&mut self.expanded_entries, other.expanded_entries);
    }
}

/// Retain the smaller of two optional project budget restrictions.
fn narrow_limit<T: Ord + Copy>(current: &mut Option<T>, candidate: Option<T>) {
    if let Some(requested) = candidate {
        *current = Some(current.map_or(requested, |existing| existing.min(requested)));
    }
}
