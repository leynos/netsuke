//! Bounded traversal for the `flatten` collection filter.
//!
//! `MiniJinja` sequences share their descendants, so a compact template graph
//! can describe an expansion far larger than the template that built it. The
//! traversal therefore charges one budget for every yielded item, every
//! retained leaf, and every level of nesting. Charging happens before the
//! traversal descends or appends, so the budget rejects a graph rather than
//! completing an expansion it cannot afford.
//!
//! One [`FlattenBudget`] is created by [`super::register_filters`] and captured
//! by the registered filter, so every `flatten` call on one environment draws
//! from the same allowance. That scope is one manifest load, which is the same
//! scope [`crate::manifest::budget::ManifestBudget`] uses; a template loop
//! cannot multiply the work by calling the filter repeatedly. The budget is not
//! global process state and tracks no object identity: the public `MiniJinja`
//! API exposes no stable identity for a host object, so a cycle is detected by
//! counting visits and depth rather than by recognizing a repeated descendant.
//!
//! Only [`super::register_filters`] may register this filter; it is the single
//! registration path for full rendering, discovery, and manifest queries.

use std::sync::atomic::{AtomicUsize, Ordering};

use minijinja::{
    Error, ErrorKind,
    value::{Value, ValueKind},
};

use crate::localization::{self, keys};

/// Fixed resource ceilings applied to one registered environment.
#[expect(
    clippy::struct_field_names,
    reason = "Each field is a ceiling, and the `max_` prefix keeps a limit \
              distinct from the count it bounds at every use site."
)]
#[derive(Clone, Copy, Debug)]
pub(super) struct FlattenLimits {
    /// Maximum depth of any item in one traversal.
    ///
    /// Immediate members of the filter subject are depth one, and each
    /// container beneath them adds one. The ceiling also bounds the native
    /// recursion: a depth of 128 keeps the recursive frames far inside every
    /// supported platform's stack, so the budget rejects a deep graph before
    /// the process can exhaust its stack.
    max_depth: usize,
    /// Maximum items yielded by all calls in the environment.
    ///
    /// This equals the default manifest evaluation fuel. `MiniJinja` fuel
    /// counts engine instructions and does not meter work inside a native
    /// filter, so the visit ceiling gives the whole environment the same order
    /// of native item visits as the engine's default instruction allowance for
    /// one evaluation.
    max_visits: usize,
    /// Maximum leaves retained by all calls in the environment.
    ///
    /// One tenth of the visit ceiling: the traversal may inspect ten items for
    /// each leaf it keeps, which still holds the result allocation to a fixed
    /// fraction of the traversal work.
    max_leaves: usize,
}

impl FlattenLimits {
    /// The production ceilings described by the field documentation.
    pub(super) const DEFAULT: Self = Self {
        max_depth: 128,
        max_visits: 1_000_000,
        max_leaves: 100_000,
    };

    /// Build a small allowance so a test can prove an exact threshold.
    ///
    /// The seam exists only for the sibling `tests` module: production code
    /// always takes [`Self::DEFAULT`], so a template cannot choose its own
    /// ceilings.
    #[cfg(test)]
    pub(super) const fn for_test(max_depth: usize, max_visits: usize, max_leaves: usize) -> Self {
        Self {
            max_depth,
            max_visits,
            max_leaves,
        }
    }
}

/// Which counter one traversal charge is applied to.
#[derive(Clone, Copy, Debug)]
enum Charge {
    /// One yielded item, container or leaf.
    Visit,
    /// One leaf retained in the result.
    Leaf,
}

/// Counters shared by every `flatten` call on one registered environment.
#[derive(Debug, Default)]
struct FlattenCounters {
    /// Items yielded by every call so far.
    visits: AtomicUsize,
    /// Leaves retained by every call so far.
    leaves: AtomicUsize,
}

/// The allowance one registered `flatten` filter draws from.
#[derive(Debug)]
pub(super) struct FlattenBudget {
    /// Ceilings this budget charges against.
    limits: FlattenLimits,
    /// Counters shared by every call.
    counters: FlattenCounters,
}

impl FlattenBudget {
    /// Create the budget for one registered environment.
    pub(super) const fn new(limits: FlattenLimits) -> Self {
        Self {
            limits,
            counters: FlattenCounters {
                visits: AtomicUsize::new(0),
                leaves: AtomicUsize::new(0),
            },
        }
    }

    /// Charge one visit or retained leaf against its ceiling.
    ///
    /// The counter is advanced before the comparison so no read-modify-write
    /// race can admit two charges for the last unit. A rejected charge leaves
    /// the counter one past its ceiling, which is visible only in accounting;
    /// the traversal aborts on the first breach.
    ///
    /// # Errors
    ///
    /// Returns an error naming the exhausted ceiling.
    fn record(&self, charge: Charge) -> Result<(), Error> {
        let (counter, ceiling, limit_key) = match charge {
            Charge::Visit => (
                &self.counters.visits,
                self.limits.max_visits,
                keys::STDLIB_COLLECTIONS_FLATTEN_VISIT_LIMIT,
            ),
            Charge::Leaf => (
                &self.counters.leaves,
                self.limits.max_leaves,
                keys::STDLIB_COLLECTIONS_FLATTEN_ITEM_LIMIT,
            ),
        };
        let charged = counter.fetch_add(1, Ordering::Relaxed) + 1;
        if charged > ceiling {
            return Err(limit_error(limit_key, ceiling));
        }
        Ok(())
    }

    /// Reject an item that would sit deeper than the nesting ceiling.
    ///
    /// # Errors
    ///
    /// Returns an error naming the depth ceiling when `depth` exceeds it.
    fn check_depth(&self, depth: usize) -> Result<(), Error> {
        if depth <= self.limits.max_depth {
            return Ok(());
        }
        Err(limit_error(
            keys::STDLIB_COLLECTIONS_FLATTEN_DEPTH_LIMIT,
            self.limits.max_depth,
        ))
    }
}

/// Flatten nested sequences within `values` into a single flat sequence.
///
/// The result is built directly from the retained leaves; it is not serialized
/// a second time, so the traversal sees each leaf exactly once.
///
/// # Errors
///
/// Returns an error when the subject cannot be iterated, when a top-level item
/// is not a sequence or iterable, when a nested iterable cannot be iterated, or
/// when `budget` is exhausted.
pub(super) fn flatten_filter(values: &Value, budget: &FlattenBudget) -> Result<Value, Error> {
    let mut flattened = Vec::new();

    for item in values.try_iter()? {
        budget.record(Charge::Visit)?;
        budget.check_depth(1)?;
        match item.kind() {
            ValueKind::Seq | ValueKind::Iterable => {
                collect_flattened_values(&item, 1, budget, &mut flattened)?;
            }
            kind => {
                return Err(Error::new(
                    ErrorKind::InvalidOperation,
                    localization::message(keys::STDLIB_COLLECTIONS_FLATTEN_EXPECTED_SEQUENCE)
                        .with_arg("kind", kind.to_string())
                        .to_string(),
                ));
            }
        }
    }

    Ok(Value::from(flattened))
}

/// Append the leaves of the container `value` to `output`.
///
/// `value` is the container itself, at `depth`; its children are one deeper.
/// Every child is charged as a visit before it is classified, and a leaf is
/// charged again before retention, so both ceilings are checked before the
/// traversal descends or the result grows.
///
/// # Errors
///
/// Returns an error when `value` cannot be iterated or when a child exceeds the
/// visit, depth, or retained-item ceiling.
fn collect_flattened_values(
    value: &Value,
    depth: usize,
    budget: &FlattenBudget,
    output: &mut Vec<Value>,
) -> Result<(), Error> {
    for nested in value.try_iter()? {
        budget.record(Charge::Visit)?;
        let child_depth = depth + 1;
        budget.check_depth(child_depth)?;
        match nested.kind() {
            ValueKind::Seq | ValueKind::Iterable => {
                collect_flattened_values(&nested, child_depth, budget, output)?;
            }
            _ => {
                budget.record(Charge::Leaf)?;
                output.push(nested);
            }
        }
    }
    Ok(())
}

/// Build a localized `InvalidOperation` error naming `limit`.
fn limit_error(key: &'static str, limit: usize) -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        localization::message(key)
            .with_arg("limit", limit)
            .to_string(),
    )
}

#[cfg(test)]
mod property;
