//! Bounded traversal for the `flatten` collection filter.
//!
//! `MiniJinja` sequences share their descendants, so a compact template graph
//! can describe an expansion far larger than the template that built it. The
//! traversal therefore charges one allowance for every yielded item, every
//! retained leaf, and every level of nesting. Charging happens before the
//! traversal descends or appends, so the allowance rejects a graph rather than
//! completing an expansion it cannot afford.
//!
//! The allowance is per filter call, is not shared across calls, and tracks no
//! object identity. The public `MiniJinja` API exposes no stable identity for
//! a host object, so a cycle is detected by counting visits and depth rather
//! than by recognizing a repeated descendant.
//!
//! Only [`super::register_filters`] may register this filter; it is the single
//! registration path for full rendering, discovery, and manifest queries.

use minijinja::{
    Error, ErrorKind,
    value::{Value, ValueKind},
};

use crate::localization::{self, keys};

/// Fixed resource ceilings applied to one `flatten` call.
#[expect(
    clippy::struct_field_names,
    reason = "Each field is a ceiling, and the `max_` prefix keeps a limit \
              distinct from the count it bounds at every use site."
)]
#[derive(Clone, Copy, Debug)]
pub(super) struct FlattenLimits {
    /// Maximum depth of any item in the traversal.
    ///
    /// Immediate members of the filter subject are depth one, and each
    /// container beneath them adds one. The ceiling also bounds the native
    /// recursion: a depth of 128 keeps the recursive frames far inside every
    /// supported platform's stack, so the allowance rejects a deep graph
    /// before the process can exhaust its stack.
    max_depth: usize,
    /// Maximum items yielded by the traversal, containers included.
    ///
    /// This equals the default manifest evaluation fuel. `MiniJinja` fuel
    /// counts engine instructions and does not meter work inside a native
    /// filter, so the visit ceiling gives one filter call the same order of
    /// native item visits as the engine's default instruction allowance for a
    /// whole evaluation.
    max_visits: usize,
    /// Maximum leaves retained in the flattened result.
    ///
    /// One tenth of the visit ceiling: a graph may inspect ten items for each
    /// leaf it keeps, which still holds the result allocation to a fixed
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

/// Mutable counters shared by one flatten traversal.
struct FlattenAllowance<'limits> {
    /// Ceilings this traversal charges against.
    limits: &'limits FlattenLimits,
    /// Items yielded so far, containers and leaves alike.
    visits: usize,
    /// Leaves retained so far.
    leaves: usize,
}

impl<'limits> FlattenAllowance<'limits> {
    /// Start a fresh allowance for one filter call.
    const fn new(limits: &'limits FlattenLimits) -> Self {
        Self {
            limits,
            visits: 0,
            leaves: 0,
        }
    }

    /// Charge one yielded item before it is classified or descended into.
    ///
    /// # Errors
    ///
    /// Returns an error naming the visit ceiling when it is exhausted.
    fn record_visit(&mut self) -> Result<(), Error> {
        if self.visits >= self.limits.max_visits {
            return Err(limit_error(
                keys::STDLIB_COLLECTIONS_FLATTEN_VISIT_LIMIT,
                self.limits.max_visits,
            ));
        }
        self.visits += 1;
        Ok(())
    }

    /// Charge one leaf before it is retained in the result.
    ///
    /// # Errors
    ///
    /// Returns an error naming the retained-item ceiling when it is exhausted.
    fn record_leaf(&mut self) -> Result<(), Error> {
        if self.leaves >= self.limits.max_leaves {
            return Err(limit_error(
                keys::STDLIB_COLLECTIONS_FLATTEN_ITEM_LIMIT,
                self.limits.max_leaves,
            ));
        }
        self.leaves += 1;
        Ok(())
    }

    /// Reject an item that would sit deeper than the nesting ceiling.
    ///
    /// # Errors
    ///
    /// Returns an error naming the depth ceiling when `depth` exceeds it.
    fn check_depth(&self, depth: usize) -> Result<(), Error> {
        if depth > self.limits.max_depth {
            return Err(limit_error(
                keys::STDLIB_COLLECTIONS_FLATTEN_DEPTH_LIMIT,
                self.limits.max_depth,
            ));
        }
        Ok(())
    }
}

/// Flatten nested sequences within `values` into a single flat sequence.
///
/// # Errors
///
/// Returns an error when the subject cannot be iterated, when a top-level item
/// is not a sequence or iterable, when a nested iterable cannot be iterated, or
/// when the traversal exceeds one of the [`FlattenLimits::DEFAULT`] ceilings.
pub(super) fn flatten_filter(values: &Value) -> Result<Value, Error> {
    flatten_filter_with_limits(values, &FlattenLimits::DEFAULT)
}

/// Flatten `values` under an explicit allowance.
///
/// The result is built directly from the retained leaves; it is not
/// serialized a second time, so the traversal sees each leaf exactly once.
///
/// # Errors
///
/// Returns an error when the subject cannot be iterated, when a top-level item
/// is not a sequence or iterable, when a nested iterable cannot be iterated, or
/// when any of `limits` is exhausted.
pub(super) fn flatten_filter_with_limits(
    values: &Value,
    limits: &FlattenLimits,
) -> Result<Value, Error> {
    let mut allowance = FlattenAllowance::new(limits);
    let mut flattened = Vec::new();

    for item in values.try_iter()? {
        allowance.record_visit()?;
        allowance.check_depth(1)?;
        match item.kind() {
            ValueKind::Seq | ValueKind::Iterable => {
                collect_flattened_values(&item, 1, &mut allowance, &mut flattened)?;
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
    allowance: &mut FlattenAllowance<'_>,
    output: &mut Vec<Value>,
) -> Result<(), Error> {
    for nested in value.try_iter()? {
        allowance.record_visit()?;
        let child_depth = depth + 1;
        allowance.check_depth(child_depth)?;
        match nested.kind() {
            ValueKind::Seq | ValueKind::Iterable => {
                collect_flattened_values(&nested, child_depth, allowance, output)?;
            }
            _ => {
                allowance.record_leaf()?;
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
