//! Parse roadmap tasks and compute their dependency relationships.

use std::collections::{BTreeMap, BTreeSet};

use regex::Regex;

/// Parse checked or unchecked roadmap tasks and their dependency bullets.
pub(super) fn roadmap_tasks(roadmap: &str) -> BTreeMap<String, BTreeSet<String>> {
    let Some(task_pattern) = Regex::new(r"^- \[[ x]\] (\d+\.\d+\.\d+)\.").ok() else {
        return BTreeMap::new();
    };
    let Some(dependency_pattern) = Regex::new(r"\b\d+\.\d+\.\d+\b").ok() else {
        return BTreeMap::new();
    };
    let mut lines = roadmap.lines().peekable();
    let mut tasks = BTreeMap::new();

    while let Some(line) = lines.next() {
        let Some(task) = task_identifier(line, &task_pattern) else {
            continue;
        };
        let dependencies = task_dependencies(&mut lines, &task_pattern, &dependency_pattern);
        tasks.insert(task, dependencies);
    }

    tasks
}

/// Extract a task identifier from a roadmap task line.
fn task_identifier(line: &str, task_pattern: &Regex) -> Option<String> {
    task_pattern
        .captures(line)
        .and_then(|captures| captures.get(1))
        .map(|capture| capture.as_str().to_owned())
}

/// Collect dependency references until the next roadmap task.
fn task_dependencies(
    lines: &mut std::iter::Peekable<std::str::Lines<'_>>,
    task_pattern: &Regex,
    dependency_pattern: &Regex,
) -> BTreeSet<String> {
    let mut dependencies = BTreeSet::new();
    while let Some(next_line) = lines.peek().copied() {
        if task_pattern.is_match(next_line) {
            break;
        }
        let Some(consumed_line) = lines.next() else {
            break;
        };
        let Some(mut dependency_text) = dependency_text(consumed_line) else {
            continue;
        };
        append_dependency_continuations(lines, task_pattern, &mut dependency_text);
        dependencies.extend(
            dependency_pattern
                .find_iter(&dependency_text)
                .map(|matched| matched.as_str().to_owned()),
        );
    }
    dependencies
}

/// Return the inline text of a two-space-indented dependency bullet.
fn dependency_text(line: &str) -> Option<String> {
    let indentation = line.len() - line.trim_start().len();
    (indentation == 2)
        .then(|| line.trim_start().strip_prefix("- Dependencies:"))
        .flatten()
        .map(str::to_owned)
}

/// Append indented continuation prose without consuming the next list item.
fn append_dependency_continuations(
    lines: &mut std::iter::Peekable<std::str::Lines<'_>>,
    task_pattern: &Regex,
    dependency_text: &mut String,
) {
    while let Some(next_line) = lines.peek().copied() {
        if task_pattern.is_match(next_line) || starts_sibling_bullet(next_line) {
            break;
        }
        let Some(consumed_line) = lines.next() else {
            break;
        };
        append_nonempty_line(dependency_text, consumed_line);
    }
}

/// Identify a sibling bullet that terminates a dependency continuation.
fn starts_sibling_bullet(line: &str) -> bool {
    let indentation = line.len() - line.trim_start().len();
    indentation <= 2 && line.trim_start().starts_with("- ")
}

/// Append nonblank continuation text to its inline dependency value.
fn append_nonempty_line(dependency_text: &mut String, line: &str) {
    let continuation = line.trim();
    if !continuation.is_empty() {
        dependency_text.push(' ');
        dependency_text.push_str(continuation);
    }
}

/// Compute the transitive set of tasks whose dependencies reach `root`.
pub(super) fn dependents_of(
    tasks: &BTreeMap<String, BTreeSet<String>>,
    root: &str,
) -> BTreeSet<String> {
    let mut dependents = BTreeSet::new();
    loop {
        let additions: BTreeSet<String> = tasks
            .iter()
            .filter(|(task, dependencies)| {
                !dependents.contains(*task)
                    && (dependencies.contains(root)
                        || dependencies
                            .iter()
                            .any(|dependency| dependents.contains(dependency)))
            })
            .map(|(task, _)| task.clone())
            .collect();
        if additions.is_empty() {
            break;
        }
        dependents.extend(additions);
    }
    dependents
}
