//! Property-test the obligation-to-trace binding over generated sections.
//!
//! `obligation_tests.rs` pins the shapes a reader wrote by hand. These
//! properties cover the input domain instead: any set of obligations and any
//! trace table written to match them.
//!
//! The binding is a relation between two documents, so one direction cannot
//! guard it. A reader that accepted every pair still passes an acceptance
//! property, and a reader that refused every pair still passes a rejection
//! property; only asserting both rules out those two mutants. The expectation
//! comes from the generator's record of what it wrote, never from the reader's
//! own predicates — the shared-oracle defect the region and harvest properties
//! were each repaired for.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use proptest::prelude::*;

use super::obligations::validate_obligation_tasks;

/// The roadmap tasks a generated obligation may bind.
///
/// `task_ids` reads `2[6-9].n.n`, so a value outside that shape is not a task
/// as far as the reader is concerned. Generating one would assert about the
/// identifier grammar instead of about the binding.
const TASKS: [&str; 4] = ["26.1.2", "26.1.3", "27.1.1", "28.2.1"];

/// The highest obligation number the generator may use.
const MAX_OBLIGATION: usize = 8;

/// One obligation and the tasks its `Tasks:` clause binds.
type Obligation = (String, BTreeSet<String>);

/// A generated document, plus the task and obligation the omission case breaks.
///
/// The pair travels with the obligations so a case cannot name one the document
/// does not bind. Drawing them separately would need a guard branch inside the
/// `proptest!` body, and a bare `return` there does not type-check.
type Binding = (Vec<Obligation>, String, String);

/// Draw a non-empty subset of `TASKS` for one obligation to bind.
fn bound_tasks() -> impl Strategy<Value = BTreeSet<String>> {
    prop::collection::btree_set(prop::sample::select(&TASKS[..]), 1..=TASKS.len())
        .prop_map(|names| names.into_iter().map(str::to_owned).collect())
}

/// Draw between one and four obligations with distinct identifiers.
///
/// Keying by number rather than by rendered identifier makes the identifiers
/// distinct by construction. Two obligations sharing `CO-1` would let one row
/// satisfy a binding the other declares, so the property would assert about a
/// document its own generator could not have meant.
fn obligations() -> impl Strategy<Value = Vec<Obligation>> {
    prop::collection::btree_map(0usize..MAX_OBLIGATION, bound_tasks(), 1..=4).prop_map(
        |by_number| {
            by_number
                .into_iter()
                .map(|(number, tasks)| (format!("CO-{}", number + 1), tasks))
                .collect()
        },
    )
}

/// Draw a document together with one of its bound pairs.
///
/// The pair is chosen after the bindings are known, so it always names a
/// binding the document actually declares. That keeps the omission case a
/// statement about every document the generator can produce, rather than about
/// the subset where an independently drawn pair happened to be present.
fn binding() -> impl Strategy<Value = Binding> {
    obligations().prop_flat_map(|obligations| {
        let pairs = bound_pairs(&obligations);
        let chosen = prop::sample::select(pairs);
        chosen.prop_map(move |(task, identifier)| (obligations.clone(), task, identifier))
    })
}

/// Render the compatibility-obligations section for `obligations`.
fn obligations_section(obligations: &[Obligation]) -> String {
    let mut text = String::new();
    for (identifier, tasks) in obligations {
        let list = list_cell(tasks);
        writeln!(text, "- {identifier}: Preserve it. Tasks: {list}.")
            .expect("writing to a String cannot fail");
    }
    text
}

/// Render a `Task`-headed trace table with one row per key in `rows`.
///
/// The section heading and the `Task`-headed table are both required: a row
/// counts as evidence only inside a `Task`-headed table under `### Changes` or
/// `### Consumes`, so a table placed anywhere else would exercise the selection
/// rule rather than the binding rule.
fn trace_section(rows: &BTreeMap<String, BTreeSet<String>>) -> String {
    let mut text = String::from(concat!(
        "### Changes\n\n",
        "| Task | Register items | Findings | Obligations |\n",
        "| -- | -- | -- | -- |\n",
    ));
    for (task, named) in rows {
        let cell = list_cell(named);
        writeln!(text, "| {task} | -- | -- | {cell} |").expect("writing to a String cannot fail");
    }
    text
}

/// Render a set of identifiers as one table cell.
///
/// An empty set renders as `--`, the inventory's own placeholder, so the cell
/// carries no identifier and the reader sees a row that names nothing.
fn list_cell(identifiers: &BTreeSet<String>) -> String {
    if identifiers.is_empty() {
        "--".to_owned()
    } else {
        identifiers.iter().cloned().collect::<Vec<_>>().join(", ")
    }
}

/// Collect the row each bound task needs, naming every obligation binding it.
///
/// This is the document the rule describes, built from the generator's record
/// rather than by asking the reader what it found in a rendered page.
fn correct_rows(obligations: &[Obligation]) -> BTreeMap<String, BTreeSet<String>> {
    let mut rows = BTreeMap::<String, BTreeSet<String>>::new();
    for (identifier, tasks) in obligations {
        for task in tasks {
            rows.entry(task.clone())
                .or_default()
                .insert(identifier.clone());
        }
    }
    rows
}

/// List every task-and-obligation pair the generator bound.
///
/// The rejection property breaks exactly one pair, so the reported failure
/// names the one it broke. Breaking more than one would let the reader report
/// a different pair from the one the assertion expects, and the assertion would
/// then fail for the wrong reason.
fn bound_pairs(obligations: &[Obligation]) -> Vec<(String, String)> {
    obligations
        .iter()
        .flat_map(|(identifier, tasks)| {
            tasks
                .iter()
                .map(move |task| (task.clone(), identifier.clone()))
        })
        .collect()
}

/// Render one failure for the property message, or an empty string on success.
fn failure_message(outcome: &anyhow::Result<()>) -> String {
    outcome
        .as_ref()
        .err()
        .map_or_else(String::new, |error| format!("{error:#}"))
}

proptest! {
    /// A trace table naming every bound obligation is accepted.
    ///
    /// The direction a binding rule must never lose: the inventory as written
    /// is the document the rule exists to approve. A reader that read the wrong
    /// column, or demanded equality with the row's own clause, fails here.
    #[test]
    fn a_complete_binding_is_accepted(obligations in obligations()) {
        let rows = correct_rows(&obligations);
        let outcome = validate_obligation_tasks(
            &obligations_section(&obligations),
            &trace_section(&rows),
        );
        prop_assert!(
            outcome.is_ok(),
            "generated binding should be accepted, but was refused: {}",
            failure_message(&outcome)
        );
    }

    /// A row that omits one bound obligation is refused, naming that pair.
    ///
    /// The direction that makes the rule worth having, and the one the older
    /// global check could not see: the obligation is still traced by another
    /// task's row, so every document-wide question about it is satisfied while
    /// this task silently drops a preservation duty. The row exists and is the
    /// declared width, so the omission is the only defect the reader may report.
    ///
    /// `binding()` names the pair after the bindings are known, so it always
    /// addresses a binding this document declares and the case needs no guard.
    #[test]
    fn an_omitted_binding_is_refused((obligations, task, identifier) in binding()) {
        let mut rows = correct_rows(&obligations);
        if let Some(named) = rows.get_mut(&task) {
            named.remove(&identifier);
        }
        let outcome = validate_obligation_tasks(
            &obligations_section(&obligations),
            &trace_section(&rows),
        );
        let message = failure_message(&outcome);
        prop_assert!(
            !message.is_empty(),
            "row for {} should be refused for omitting {}, but was accepted",
            task,
            identifier
        );
        prop_assert!(
            message.contains(&task),
            "error should name task {}: {}",
            task,
            message
        );
        prop_assert!(
            message.contains(&identifier),
            "error should name obligation {}: {}",
            identifier,
            message
        );
    }
}
