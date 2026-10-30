//! How an exempt field is *used*, which is what the exemption actually claims.
//!
//! `PERMITTED_UNORDERED_FIELDS` says an exempt collection may be held because it
//! is only ever asked whether a member has been staged. `scan.rs` cannot check
//! that claim: it classifies the collection a source *names*, and a loop over
//! `&stages.staged_sidecars` names no collection at all. A caller could
//! therefore iterate the exempt field, write each path into the emitted text,
//! and trip no rule — the declaration is permitted, so nothing looks again.
//!
//! This module closes that gap by reading the uses rather than the type. The
//! rule is deliberately narrow: the only use permitted is a `insert` call on the
//! field. Everything else is a use the exemption does not cover, whether it
//! borrows the field, iterates it with `for`, calls an iterator method on it, or
//! hands it to something else as an alias — because any of those can put a hash
//! iteration order into the emitted bytes.
//!
//! It is a separate file rather than more of `scan.rs` for the reason the
//! repository applies everywhere: `scan.rs` is close to the line cap, and the
//! two questions have different subjects. `scan.rs` asks what a tree *declares*;
//! this asks how one named field is *read*.
//!
//! # What the exemption has to survive
//!
//! Three shapes would otherwise slip past a purely expression-shaped visitor.
//! A macro body is tokens rather than a tree, so a field named inside
//! `writeln!` or `format!` is not an expression node until something parses it.
//! A destructuring pattern names a field without an [`ExprField`] existing at
//! all. And a `#[cfg(test)]` item cannot reach an emitted byte, so a use inside
//! one is not a use — the same strip `scan.rs` applies, since a rule that
//! excused test code in one half of the contract and reported it in the other
//! would be two rules.

use super::macro_body;
use super::test_only;
use syn::{
    Expr, ExprField, ExprMethodCall, FieldPat, Ident, Item, Macro, Member,
    visit::{self, Visit},
};

/// The one method an exempt field may be the receiver of.
///
/// `HashSet::insert` returns whether the value was newly present and imposes no
/// order, which is the whole of the exemption's justification.
pub(super) const MEMBERSHIP_METHOD: &str = "insert";

/// One way an exempt field is read other than by asking membership.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Use {
    /// The field is read as a value: borrowed, iterated by `for`, or aliased.
    AsValue,
    /// The field is the receiver of a method other than [`MEMBERSHIP_METHOD`].
    Method(String),
}

/// Every use of `field` in `file` other than a membership call.
///
/// Returned rather than asserted so the contract states the rule and the scan
/// states the finding, which is how the rest of this crate's boundary evidence
/// is arranged.
pub(super) fn uses(file: &syn::File, field: &str) -> Vec<Use> {
    let mut scan = Uses {
        field,
        found: Vec::new(),
    };
    scan.visit_file(file);
    scan.found
}

/// The visitor that collects [`Use`]s for one field name.
struct Uses<'a> {
    /// The field name the exemption names.
    field: &'a str,
    /// Uses found so far, in visit order.
    found: Vec<Use>,
}

impl Uses<'_> {
    /// Return whether a field member names the exempt field.
    ///
    /// Compared as an identifier rather than as text: `staged_sidecar` and
    /// `staged_sidecars_extra` are different fields, and a rule that matched
    /// prefixes would excuse them.
    fn names_field(&self, member: &Member) -> bool {
        matches!(member, Member::Named(name) if name == self.field)
    }
}

impl<'ast> Visit<'ast> for Uses<'_> {
    /// Visit `item` unless it is compiled only for tests.
    ///
    /// `scan.rs` answers the same question with the same function for its own
    /// visitor. Without this override the exempt field could be iterated inside
    /// an inline `#[cfg(test)] mod` and reported as a production use, which
    /// would be a finding no author could satisfy: the code it names is never
    /// compiled into the artefact the contract protects.
    fn visit_item(&mut self, item: &'ast Item) {
        if !test_only::is_test_only(item) {
            visit::visit_item(self, item);
        }
    }

    /// Classify a field access that is the receiver of a method call.
    ///
    /// The default descent would reach the receiver's [`ExprField`] and record
    /// it as a value use, so a permitted `insert` call would be reported against
    /// itself. The call is classified here instead, and only its arguments are
    /// descended into.
    fn visit_expr_method_call(&mut self, node: &'ast ExprMethodCall) {
        if let Expr::Field(receiver) = &*node.receiver
            && self.names_field(&receiver.member)
        {
            if !names_method(&node.method, MEMBERSHIP_METHOD) {
                self.found.push(Use::Method(node.method.to_string()));
            }
            for argument in &node.args {
                self.visit_expr(argument);
            }
            return;
        }
        visit::visit_expr_method_call(self, node);
    }

    /// Record a field access that reached here, which is not a membership call.
    ///
    /// A `for` loop over `&field`, a borrow passed on as an alias, and the
    /// receiver of a call the method arm did not classify all land here, because
    /// each of them is an [`ExprField`] and none of them is `field.insert(..)`.
    fn visit_expr_field(&mut self, node: &'ast ExprField) {
        if self.names_field(&node.member) {
            self.found.push(Use::AsValue);
        }
        visit::visit_expr_field(self, node);
    }

    /// Read a macro body, which the default descent sees only as tokens.
    ///
    /// A body that parses as an expression list is descended into like any
    /// other code, so `writeln!(out, "{:?}", stages.staged_sidecars)` is caught
    /// by the same [`ExprField`] rule as an ordinary field access. A body with a
    /// grammar of its own — `matches!`, `vec![a; n]` — cannot be read that way,
    /// so it is judged on whether it spells the field at all. Reporting a
    /// spelling that turns out not to be a use is a failure a human can dismiss
    /// from the message; missing a real one is the hole this guard exists to
    /// close, and the two mistakes are not equally cheap.
    fn visit_macro(&mut self, node: &'ast Macro) {
        if let Some(arguments) = macro_body::arguments(node) {
            for argument in &arguments {
                self.visit_expr(argument);
            }
        } else if macro_body::spells(node, self.field) {
            self.found.push(Use::AsValue);
        }
        visit::visit_path(self, &node.path);
    }

    /// Record a destructured binding whose field is the exempt one.
    ///
    /// `let SerialStages { staged_sidecars, .. } = stages;` moves the collection
    /// out under a new name without ever writing an [`ExprField`], so the
    /// expression rule above cannot see it. The pattern binds the *contents* of
    /// the field to a local, which is a borrow of the field by another name, and
    /// a later iteration over that local is exactly the use the exemption does
    /// not cover.
    fn visit_field_pat(&mut self, node: &'ast FieldPat) {
        if self.names_field(&node.member) {
            self.found.push(Use::AsValue);
        }
        visit::visit_field_pat(self, node);
    }
}

/// Return whether a called method is the one called `wanted`.
///
/// A method call's name is a bare identifier, and the comparison is kept exact
/// so a method merely ending in the same letters is a different method rather
/// than a longer spelling of this one.
fn names_method(method: &Ident, wanted: &str) -> bool {
    method == wanted
}

#[cfg(test)]
mod tests {
    //! Unit tests for the use classifier, written as source text.
    //!
    //! Each case parses a small function and states what the visitor found, so
    //! the rule is read off syntax rather than off token spelling. Exactness is
    //! asserted on both subjects the rule compares: a field whose name only
    //! begins with the exempt one is a different field, and a method that only
    //! begins with the permitted spelling is a different method. Either match
    //! made lenient would excuse a use the exemption does not cover.

    use super::{Use, uses};

    /// Parse and collect, so each case reads as source text.
    fn found(source: &str, field: &str) -> Vec<Use> {
        let parsed = syn::parse_file(source).expect("the case is a valid source");
        uses(&parsed, field)
    }

    /// A membership call is the exemption's whole justification.
    #[test]
    fn a_membership_call_is_permitted() {
        let source = "fn f(stages: &mut S, path: String) { stages.staged_sidecars.insert(path); }";
        assert!(
            found(source, "staged_sidecars").is_empty(),
            "asking whether a member is present imposes no order"
        );
    }

    /// The planted output-writing iteration the assessment asks for: a loop over
    /// the exempt field is a use the exemption does not cover, even though it
    /// names no collection and would trip no declaration rule.
    #[test]
    fn an_iteration_that_writes_output_is_a_use() {
        let source = "
            fn f(stages: &S, out: &mut String) {
                for path in &stages.staged_sidecars {
                    out.push_str(path.as_str());
                }
            }
        ";
        assert_eq!(
            found(source, "staged_sidecars"),
            [Use::AsValue],
            "a loop over the exempt field can put a hash iteration order into the emitted text"
        );
    }

    /// An iterator method is not a membership question, whatever it is called.
    #[test]
    fn an_iterator_method_is_a_use() {
        let source = "fn f(stages: &S) { let n = stages.staged_sidecars.iter().count(); }";
        assert_eq!(
            found(source, "staged_sidecars"),
            [Use::Method("iter".into())]
        );
    }

    /// A borrow handed to another function is the alias the rule must catch:
    /// the callee may iterate it, and nothing at the call site says so.
    #[test]
    fn an_alias_of_the_field_is_a_use() {
        let source = "fn f(stages: &S) { let seen = &stages.staged_sidecars; }";
        assert_eq!(found(source, "staged_sidecars"), [Use::AsValue]);
    }

    /// A field whose name merely starts with the exempt one is a different
    /// field, so it is neither permitted nor reported.
    #[test]
    fn a_differently_named_field_is_not_the_exempt_one() {
        let source = "fn f(stages: &S) { for path in &stages.staged_sidecars_extra { } }";
        assert!(
            found(source, "staged_sidecars").is_empty(),
            "the exemption names one field, not a prefix"
        );
    }

    /// The method-name comparison is exact, so a longer spelling is a different
    /// method and therefore a use.
    #[test]
    fn a_longer_method_name_is_not_membership() {
        let source =
            "fn f(stages: &mut S, path: String) { stages.staged_sidecars.insert_all(path); }";
        assert_eq!(
            found(source, "staged_sidecars"),
            [Use::Method("insert_all".into())],
            "a method merely starting with the permitted spelling is a different method"
        );
    }

    /// A field named inside a macro argument is a use.
    ///
    /// `writeln!` is the shape that matters here: six calls in `staging.rs` sit
    /// beside the exempt field, and a format argument naming it would write a
    /// hash iteration order into the sidecar. The macro body is tokens, so
    /// without a reader for them this source reports nothing at all.
    #[test]
    fn a_macro_argument_that_reads_the_field_is_a_use() {
        let source = r#"fn f(stages: &S, out: &mut String) { writeln!(out, "{:?}", stages.staged_sidecars); }"#;
        assert_eq!(
            found(source, "staged_sidecars"),
            [Use::AsValue],
            "the format argument reaches the exempt field through the macro body"
        );
    }

    /// A macro body with its own grammar is still read for the field name.
    ///
    /// The body of `matches!` is not an expression list, so parsing it as one
    /// fails. A reader that treated the failure as "nothing here" would make
    /// every such macro a blind spot, which is the hole this guard exists to
    /// close rather than a shape it may accept.
    #[test]
    fn an_unparseable_macro_body_is_still_read() {
        let source =
            "fn f(stages: &S) { assert!(matches!(stages, S { staged_sidecars: ref _s })); }";
        assert!(
            found(source, "staged_sidecars").contains(&Use::AsValue),
            "a body that does not parse must still be judged on what it spells"
        );
    }

    /// A destructuring pattern names the field without any field access.
    ///
    /// `let SerialStages { staged_sidecars, .. } = stages;` binds the collection
    /// to a local, and a later `for` over that local is a borrow of the field
    /// under another name. No `ExprField` exists for the pattern, so only the
    /// pattern hook can see it.
    #[test]
    fn a_destructured_field_binding_is_a_use() {
        let source = "fn f(stages: &S) { let S { staged_sidecars } = stages; }";
        assert_eq!(
            found(source, "staged_sidecars"),
            [Use::AsValue],
            "destructuring moves the collection out under a name the rule cannot follow"
        );
    }

    /// A use inside a test-only item is not a use.
    ///
    /// The same strip `scan::collect` applies, for the same reason: a release
    /// build never compiles the item, so nothing it does can reach an emitted
    /// byte. A property test that iterated the exempt field would otherwise be
    /// a finding no author could satisfy.
    #[test]
    fn a_test_only_use_is_ignored() {
        let source = "
            #[cfg(test)]
            mod tests {
                fn f(stages: &S) {
                    for path in &stages.staged_sidecars { }
                }
            }
        ";
        assert!(
            found(source, "staged_sidecars").is_empty(),
            "test-only code cannot reach the emitted file"
        );
    }
}
