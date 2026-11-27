//! The closed set of condition shapes a generated manifest may carry.

use super::Bindings;
use proptest::prelude::*;

/// Name the variable a slot stands for.
pub(super) fn var_name(slot: usize) -> &'static str {
    match slot {
        0 => "w0",
        1 => "w1",
        2 => "w2",
        _ => "w3",
    }
}
use proptest::strategy::ValueTree;
use std::cmp::Ordering;

/// Names a condition may reference.
pub(in crate::manifest::expand::property_tests) const VAR_NAMES: [&str; 4] =
    ["w0", "w1", "w2", "w3"];

/// One comparison operator, spelled as `MiniJinja` spells it.
#[derive(Clone, Copy, Debug)]
pub(in crate::manifest::expand::property_tests) enum CmpOp {
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `==`
    Eq,
    /// `!=`
    Ne,
    /// `>`
    Gt,
    /// `>=`
    Ge,
}

/// The spelling and acceptance test that define one operator.
struct OperatorSpec {
    /// The operator as `MiniJinja` spells it.
    spelling: &'static str,
    /// Whether an ordering satisfies the operator.
    accepts: fn(Ordering) -> bool,
}

impl CmpOp {
    /// Generate one operator.
    pub(super) fn strategy() -> impl Strategy<Value = Self> {
        prop_oneof![
            Just(Self::Lt),
            Just(Self::Le),
            Just(Self::Eq),
            Just(Self::Ne),
            Just(Self::Gt),
            Just(Self::Ge),
        ]
    }

    /// Look up this operator's single description.
    const fn spec(self) -> OperatorSpec {
        match self {
            Self::Lt => OperatorSpec {
                spelling: "<",
                accepts: Ordering::is_lt,
            },
            Self::Le => OperatorSpec {
                spelling: "<=",
                accepts: Ordering::is_le,
            },
            Self::Eq => OperatorSpec {
                spelling: "==",
                accepts: Ordering::is_eq,
            },
            Self::Ne => OperatorSpec {
                spelling: "!=",
                accepts: Ordering::is_ne,
            },
            Self::Gt => OperatorSpec {
                spelling: ">",
                accepts: Ordering::is_gt,
            },
            Self::Ge => OperatorSpec {
                spelling: ">=",
                accepts: Ordering::is_ge,
            },
        }
    }

    /// Render the operator's source spelling.
    pub(super) const fn render(self) -> &'static str {
        self.spec().spelling
    }

    /// Evaluate the operator over two integers.
    pub(super) fn apply(self, lhs: i64, rhs: i64) -> bool {
        (self.spec().accepts)(lhs.cmp(&rhs))
    }
}

/// One shape a generated condition may take.
#[derive(Clone, Copy, Debug)]
pub(in crate::manifest::expand::property_tests) enum Condition {
    /// A literal boolean, written `true` or `false`.
    Literal(bool),
    /// A bare variable holding a boolean.
    Flag(usize),
    /// The negation of a boolean variable.
    NotFlag(usize),
    /// A variable holding an integer, used as a truthiness test.
    Count(usize),
    /// An integer variable compared against a small constant.
    VarCmp(usize, CmpOp, i64),
    /// `index` compared against a small constant; foreach entries only.
    IndexCmp(CmpOp, i64),
    /// `item` compared against a small constant; integer items only.
    ItemCmp(CmpOp, i64),
}

impl Condition {
    /// Generate one shape from the closed set.
    pub(super) fn strategy() -> impl Strategy<Value = Self> {
        let operator = CmpOp::strategy().boxed();
        // The two iteration shapes carry extra weight: they are only legal on
        // a foreach entry, and the coverage tripwire requires each to reach
        // five percent of *all* entry draws, most of which are static.
        // The comparison shapes carry more weight than the bare-variable ones
        // because only they can fail for a fraction of values: a literal or a
        // flag yields one verdict per draw, so an unlucky sample can starve a
        // verdict class that the once-per-draw floor requires.
        prop_oneof![
            1 => any::<bool>().prop_map(Self::Literal),
            1 => (0..VAR_NAMES.len()).prop_map(Self::Flag),
            1 => (0..VAR_NAMES.len()).prop_map(Self::NotFlag),
            1 => (0..VAR_NAMES.len()).prop_map(Self::Count),
            4 => ((0..VAR_NAMES.len()), operator.clone(), -2i64..10)
                .prop_map(|(slot, drawn_operator, constant)| {
                    Self::VarCmp(slot, drawn_operator, constant)
                }),
            5 => (operator.clone(), -2i64..10)
                .prop_map(|(drawn_operator, constant)| Self::IndexCmp(drawn_operator, constant)),
            5 => (operator, -2i64..10)
                .prop_map(|(drawn_operator, constant)| Self::ItemCmp(drawn_operator, constant)),
        ]
    }

    /// Report the variable name this shape reads, when it reads one.
    pub(in crate::manifest::expand::property_tests) fn variable(self) -> Option<&'static str> {
        match self {
            Self::Flag(slot) | Self::NotFlag(slot) | Self::Count(slot) | Self::VarCmp(slot, ..) => {
                Some(var_name(slot))
            }
            Self::Literal(_) | Self::IndexCmp(..) | Self::ItemCmp(..) => None,
        }
    }

    /// Report whether the shape reads `index`, so it cannot suit a static entry.
    pub(in crate::manifest::expand::property_tests) const fn reads_index(self) -> bool {
        matches!(self, Self::IndexCmp(..))
    }

    /// Report whether the shape reads `item`, so it cannot suit a static entry.
    pub(in crate::manifest::expand::property_tests) const fn reads_item(self) -> bool {
        matches!(self, Self::ItemCmp(..))
    }

    /// Name the shape's family for the coverage tally.
    pub(in crate::manifest::expand::property_tests) const fn family(self) -> &'static str {
        match self {
            Self::Literal(_) => "shape-literal",
            Self::Flag(_) => "shape-flag",
            Self::NotFlag(_) => "shape-not-flag",
            Self::Count(_) => "shape-count",
            Self::VarCmp(..) => "shape-var-cmp",
            Self::IndexCmp(..) => "shape-index-cmp",
            Self::ItemCmp(..) => "shape-item-cmp",
        }
    }

    /// Report whether the shape reads either iteration binding.
    pub(in crate::manifest::expand::property_tests) const fn reads_iteration(self) -> bool {
        self.reads_index() || self.reads_item()
    }

    /// Report whether the shape has a template form at all.
    pub(in crate::manifest::expand::property_tests) const fn has_template_form(self) -> bool {
        !matches!(self, Self::Count(_))
    }

    /// Evaluate the shape in Rust, without `MiniJinja`.
    pub(in crate::manifest::expand::property_tests) fn holds(
        self,
        bindings: &Bindings,
        item: Option<i64>,
        index: usize,
    ) -> bool {
        match self {
            Self::Literal(value) => value,
            Self::Flag(slot) => bindings.flag(slot),
            Self::NotFlag(slot) => !bindings.flag(slot),
            Self::Count(slot) => bindings.count(slot) != 0,
            Self::VarCmp(slot, op, constant) => op.apply(bindings.count(slot), constant),
            Self::IndexCmp(op, constant) => {
                op.apply(i64::try_from(index).unwrap_or(i64::MAX), constant)
            }
            Self::ItemCmp(op, constant) => item.is_some_and(|value| op.apply(value, constant)),
        }
    }

    /// Render the shape's expression source.
    pub(in crate::manifest::expand::property_tests) fn render(self) -> String {
        match self {
            Self::Literal(value) => value.to_string(),
            Self::Flag(slot) | Self::Count(slot) => var_name(slot).to_owned(),
            Self::NotFlag(slot) => format!("not {}", var_name(slot)),
            Self::VarCmp(slot, op, constant) => {
                format!("{} {} {constant}", var_name(slot), op.render())
            }
            Self::IndexCmp(op, constant) => format!("index {} {constant}", op.render()),
            Self::ItemCmp(op, constant) => format!("item {} {constant}", op.render()),
        }
    }
}

#[cfg(test)]
mod tests {
    //! Reachability check for the closed set of shapes.

    use super::*;
    use proptest::test_runner::TestRunner;
    use std::collections::BTreeSet;

    /// Name the shape's class for the reachability tally.
    fn shape_class(shape: Condition) -> &'static str {
        match shape {
            Condition::Literal(_) => "literal",
            Condition::Flag(_) => "flag",
            Condition::NotFlag(_) => "not-flag",
            Condition::Count(_) => "count",
            Condition::VarCmp(..) => "var-cmp",
            Condition::IndexCmp(..) => "index-cmp",
            Condition::ItemCmp(..) => "item-cmp",
        }
    }

    /// Verify the shape renders the binding it claims to read.
    fn check_reads(shape: Condition) -> Result<(), String> {
        if shape.reads_index() && !shape.render().starts_with("index") {
            return Err(format!(
                "{shape:?} reads index but renders {}",
                shape.render()
            ));
        }
        if shape.reads_item() && !shape.render().starts_with("item") {
            return Err(format!(
                "{shape:?} reads item but renders {}",
                shape.render()
            ));
        }
        if shape.has_template_form() || matches!(shape, Condition::Count(_)) {
            return Ok(());
        }
        Err(format!(
            "unexpected shape without a template form: {shape:?}"
        ))
    }

    #[test]
    fn every_shape_names_its_reads() -> Result<(), String> {
        let mut runner = TestRunner::deterministic();
        let strategy = Condition::strategy();
        let mut seen = BTreeSet::new();
        for _ in 0..256 {
            let shape = strategy
                .new_tree(&mut runner)
                .map_err(|error| error.to_string())?
                .current();
            seen.insert(shape_class(shape));
            check_reads(shape)?;
        }
        let expected = BTreeSet::from([
            "count",
            "flag",
            "index-cmp",
            "item-cmp",
            "literal",
            "not-flag",
            "var-cmp",
        ]);
        if seen != expected {
            return Err(format!("reached {seen:?}, expected {expected:?}"));
        }
        Ok(())
    }
}
