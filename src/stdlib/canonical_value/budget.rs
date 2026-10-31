//! Bounded resource accounting for one canonical-admission traversal.
//!
//! Admitting a `MiniJinja` value materializes every collection occurrence into
//! its own snapshot subtree. A compact host graph that shares a descendant
//! between two parents therefore costs work proportional to the *expanded*
//! tree rather than to the number of distinct host objects: a depth-64 chain of
//! two-item sequences is 65 objects but `2^65 - 1` occurrences. This module
//! supplies the traversal budget that keeps that cost bounded and turns an
//! over-budget traversal into a typed refusal instead of an allocation the
//! caller never asked for.
//!
//! The budget is occurrence-based. The root and every enumerated item or pair
//! are charged one node, so the node charge equals the number of snapshot
//! nodes a completed admission would allocate. String scalars and member names
//! are charged their UTF-8 byte length before the owning copy, so an oversized
//! string is refused before it is duplicated. Depth is a bound rather than a
//! counter: it is checked before a container enumerates, which also caps the
//! recursion depth of snapshot serialization and teardown.
//!
//! The limits are deliberately not configurable. They are a safety bound on a
//! public library surface rather than an operator policy, and the supported
//! input domain sits far below every default; a caller needing a different
//! ceiling needs a different API, not a different number.

use super::error::{CanonicalResource, CanonicalValueError, SnapshotPath};

/// Default maximum distance from the snapshot root to any admitted value.
///
/// Deep enough for any manifest-shaped value and shallow enough that snapshot
/// serialization and teardown cannot exhaust the native stack.
pub(super) const DEFAULT_MAX_DEPTH: usize = 128;

/// Default maximum snapshot nodes charged by one public operation.
///
/// Counted per collection occurrence, so a shared host graph is charged the
/// expansion it asks for rather than the number of objects it holds.
pub(super) const DEFAULT_MAX_NODES: usize = 100_000;

/// Default maximum bytes of admitted strings and member names, in UTF-8.
pub(super) const DEFAULT_MAX_BYTES: usize = 8 * 1024 * 1024;

/// Ceilings applied to one canonical admission.
///
/// Each field is a maximum: the struct name supplies the "limit" qualifier, so
/// the fields stay free of a repeated prefix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct AdmissionLimits {
    /// Maximum depth below the snapshot root.
    pub(super) depth: usize,
    /// Maximum snapshot nodes, counted per occurrence.
    pub(super) nodes: usize,
    /// Maximum admitted string and member-name bytes.
    pub(super) bytes: usize,
}

impl Default for AdmissionLimits {
    /// Return the safe production ceilings.
    fn default() -> Self {
        Self {
            depth: DEFAULT_MAX_DEPTH,
            nodes: DEFAULT_MAX_NODES,
            bytes: DEFAULT_MAX_BYTES,
        }
    }
}

/// Consumed and remaining accounting for one admission traversal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct AdmissionBudget {
    /// Ceilings this budget enforces.
    limits: AdmissionLimits,
    /// Snapshot nodes charged so far, including the root.
    used_nodes: usize,
    /// String and member-name bytes charged so far.
    used_bytes: usize,
}

impl AdmissionBudget {
    /// Start a fresh traversal under `limits`.
    pub(super) const fn new(limits: AdmissionLimits) -> Self {
        Self {
            limits,
            used_nodes: 0,
            used_bytes: 0,
        }
    }

    /// Return how many further nodes this traversal may charge.
    ///
    /// A bounded enumerator takes one item more than this, because the extra
    /// item is what turns an over-long or endless enumeration into a typed
    /// refusal instead of silently truncating it.
    pub(super) const fn remaining_nodes(&self) -> usize {
        self.limits.nodes.saturating_sub(self.used_nodes)
    }

    /// Refuse a value more than `depth` edges below the snapshot root.
    ///
    /// Checked for every value rather than only for containers, so a scalar at
    /// an over-deep position is refused by the same rule that bounds recursion.
    pub(super) fn check_depth(
        &self,
        depth: usize,
        path: &SnapshotPath,
    ) -> Result<(), CanonicalValueError> {
        if depth > self.limits.depth {
            return Err(CanonicalValueError::resource_limit(
                CanonicalResource::Depth,
                self.limits.depth,
                path,
            ));
        }
        Ok(())
    }

    /// Charge one snapshot node, refusing when the ceiling is already reached.
    ///
    /// A charge that lands exactly on the ceiling succeeds; only the next one
    /// is refused, which keeps the limit an inclusive allowance.
    pub(super) fn charge_node(&mut self, path: &SnapshotPath) -> Result<(), CanonicalValueError> {
        Allowance {
            used: &mut self.used_nodes,
            limit: self.limits.nodes,
            resource: CanonicalResource::Nodes,
        }
        .charge(1, path)
    }

    /// Charge `bytes` of admitted text before it is copied.
    pub(super) fn charge_bytes(
        &mut self,
        bytes: usize,
        path: &SnapshotPath,
    ) -> Result<(), CanonicalValueError> {
        Allowance {
            used: &mut self.used_bytes,
            limit: self.limits.bytes,
            resource: CanonicalResource::Bytes,
        }
        .charge(bytes, path)
    }
}

impl Default for AdmissionBudget {
    /// Start a fresh traversal under the production ceilings.
    fn default() -> Self {
        Self::new(AdmissionLimits::default())
    }
}

/// One counter with the ceiling and resource that bound it.
///
/// Grouping the three keeps a single charge site for both node and byte
/// accounting, so the checked arithmetic and the refusal shape are written
/// once.
struct Allowance<'a> {
    /// Counter this allowance accumulates into.
    used: &'a mut usize,
    /// Ceiling `used` may reach.
    limit: usize,
    /// Resource named by a refusal from this allowance.
    resource: CanonicalResource,
}

impl Allowance<'_> {
    /// Add `amount`, refusing without changing the counter when over the limit.
    ///
    /// A charge that lands exactly on the ceiling succeeds. The counter is left
    /// untouched on refusal, so a caller that recovers from one refusal sees the
    /// same remaining allowance it saw before the attempt.
    fn charge(&mut self, amount: usize, path: &SnapshotPath) -> Result<(), CanonicalValueError> {
        let Some(next) = self.used.checked_add(amount) else {
            return Err(self.refusal(path));
        };
        if next > self.limit {
            return Err(self.refusal(path));
        }
        *self.used = next;
        Ok(())
    }

    /// Build the refusal this allowance reports at `path`.
    fn refusal(&self, path: &SnapshotPath) -> CanonicalValueError {
        CanonicalValueError::resource_limit(self.resource, self.limit, path)
    }
}
