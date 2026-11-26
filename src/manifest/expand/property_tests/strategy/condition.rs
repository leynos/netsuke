//! The closed set of condition shapes a generated manifest may carry.
//!
//! A recursive expression grammar would put `MiniJinja`'s evaluator under test
//! rather than expansion, and would bring an oracle that has to re-implement
//! it. The shapes here are the ones the expansion contract actually depends
//! on: a literal, a boolean variable, its negation, an integer used as a
//! truthiness test, an integer comparison against a variable, and comparisons
//! against `index` and `item`. Each has a one-line Rust meaning, so the oracle
//! shares no code with the evaluator.
//!
//! Every shape except [`Condition::Count`] also has a template form. `Count` is
//! deliberately excluded: a template renders a string, and the fallback only
//! accepts `true` or `1`, so a rendered integer would silently read as false
//! and the oracle would no longer describe the production path.

use super::Bindings;
use proptest::prelude::*;
use proptest::strategy::ValueTree;

/// Names a condition may reference.
///
/// None of these can collide with a `MiniJinja` keyword, so a generated
/// identifier is always read as a variable rather than as syntax.
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

    /// Render the operator's source spelling.
    pub(super) const fn render(self) -> &'static str {
        match self {
            Self::Lt => "<",
            Self::Le => "<=",
            Self::Eq => "==",
            Self::Ne => "!=",
            Self::Gt => ">",
            Self::Ge => ">=",
        }
    }

    /// Evaluate the operator over two integers.
    pub(super) const fn apply(self, lhs: i64, rhs: i64) -> bool {
        match self {
            Self::Lt => lhs < rhs,
            Self::Le => lhs <= rhs,
            Self::Eq => lhs == rhs,
            Self::Ne => lhs != rhs,
            Self::Gt => lhs > rhs,
            Self::Ge => lhs >= rhs,
        }
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
        let op = CmpOp::strategy().boxed();
        // The two iteration shapes carry extra weight: they are only legal on
        // a foreach entry, and the coverage tripwire requires each to reach
        // five percent of *all* entry draws, most of which are static.
        prop_oneof![
            1 => any::<bool>().prop_map(Self::Literal),
            2 => (0..VAR_NAMES.len()).prop_map(Self::Flag),
            2 => (0..VAR_NAMES.len()).prop_map(Self::NotFlag),
            2 => (0..VAR_NAMES.len()).prop_map(Self::Count),
            2 => ((0..VAR_NAMES.len()), op.clone(), -2i64..10)
                .prop_map(|(slot, op, constant)| Self::VarCmp(slot, op, constant)),
            3 => (op.clone(), -2i64..10).prop_map(|(op, constant)| Self::IndexCmp(op, constant)),
            3 => (op, -2i64..10).prop_map(|(op, constant)| Self::ItemCmp(op, constant)),
        ]
    }

    /// Report the variable name this shape reads, when it reads one.
    pub(in crate::manifest::expand::property_tests) fn variable(self) -> Option<&'static str> {
        match self {
            Self::Flag(slot) | Self::NotFlag(slot) | Self::Count(slot) | Self::VarCmp(slot, ..) => {
                Some(VAR_NAMES[slot])
            }
            Self::Literal(_) | Self::IndexCmp(..) | Self::ItemCmp(..) => None,
        }
    }

    /// Report whether the shape reads `index`, and so cannot appear on a
    /// static entry.
    pub(in crate::manifest::expand::property_tests) const fn reads_index(self) -> bool {
        matches!(self, Self::IndexCmp(..))
    }

    /// Report whether the shape reads `item`, and so cannot appear on a
    /// static entry.
    pub(in crate::manifest::expand::property_tests) const fn reads_item(self) -> bool {
        matches!(self, Self::ItemCmp(..))
    }

    /// Report whether the shape has a template form at all.
    pub(in crate::manifest::expand::property_tests) const fn has_template_form(self) -> bool {
        !matches!(self, Self::Count(_))
    }

    /// Evaluate the shape in Rust, without `MiniJinja`.
    ///
    /// `item` is the iteration value read as an integer, and is `None` when
    /// the value is not an integer; a shape that never reads `item` ignores
    /// it. `index` is the zero-based iteration position.
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
            Self::IndexCmp(op, constant) => op.apply(index as i64, constant),
            Self::ItemCmp(op, constant) => item.is_some_and(|value| op.apply(value, constant)),
        }
    }

    /// Render the shape's expression source.
    pub(in crate::manifest::expand::property_tests) fn render(self) -> String {
        match self {
            Self::Literal(value) => value.to_string(),
            Self::Flag(slot) => VAR_NAMES[slot].to_owned(),
            Self::NotFlag(slot) => format!("not {}", VAR_NAMES[slot]),
            Self::Count(slot) => VAR_NAMES[slot].to_owned(),
            Self::VarCmp(slot, op, constant) => {
                format!("{} {} {constant}", VAR_NAMES[slot], op.render())
            }
            Self::IndexCmp(op, constant) => format!("index {} {constant}", op.render()),
            Self::ItemCmp(op, constant) => format!("item {} {constant}", op.render()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::test_runner::TestRunner;
    use std::collections::BTreeSet;

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
            let label = match shape {
                Condition::Literal(_) => "literal",
                Condition::Flag(_) => "flag",
                Condition::NotFlag(_) => "not-flag",
                Condition::Count(_) => "count",
                Condition::VarCmp(..) => "var-cmp",
                Condition::IndexCmp(..) => "index-cmp",
                Condition::ItemCmp(..) => "item-cmp",
            };
            seen.insert(label);
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
            if !shape.has_template_form() && !matches!(shape, Condition::Count(_)) {
                return Err(format!(
                    "unexpected shape without a template form: {shape:?}"
                ));
            }
        }
        assert_eq!(
            seen,
            BTreeSet::from([
                "count",
                "flag",
                "index-cmp",
                "item-cmp",
                "literal",
                "not-flag",
                "var-cmp",
            ])
        );
        Ok(())
    }
}
