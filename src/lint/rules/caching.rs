//! Rules about declarations that defeat change detection.
//!
//! Both rules here describe the same failure: a dependency edge whose
//! timestamp moves for reasons unrelated to the dependent target's inputs, so
//! the target rebuilds on every invocation and everything downstream of it
//! rebuilds too.

use std::collections::BTreeMap;

use crate::ast::{NetsukeManifest, Recipe, StringOrList, Target};
use crate::lint::document::Node;
use crate::lint::registry::Registered;
use crate::lint::resolve::{self, Provenance};
use crate::lint::rule::{Category, FindingSink, ManifestContext, ManifestRule, RuleMeta, Stage};
use crate::lint::severity::{DefaultSeverity, Severity};

use super::shellscan;

/// Register this module's rules.
#[must_use]
pub fn rules() -> Vec<Registered> {
    vec![
        Registered::Manifest(&PhonyDepOfFileTarget),
        Registered::Manifest(&DirectoryDepNotOrderOnly),
    ]
}

/// The dependency keys whose entries participate in change detection.
static CONTENT_KEYS: [&str; 2] = ["sources", "deps"];

/// Detects a file target depending on an always-dirty phony target.
pub struct PhonyDepOfFileTarget;

/// Metadata for [`PhonyDepOfFileTarget`].
static PHONY_DEP_OF_FILE_TARGET: RuleMeta = RuleMeta {
    name: "phony-dep-of-file-target",
    category: Category::Caching,
    stage: Stage::Manifest,
    default_severity: DefaultSeverity::On(Severity::Warning),
    summary: "file target depends on a phony target through `sources` or `deps`",
    rationale: concat!(
        "A phony target is always considered out of date. A file target that ",
        "depends on one through a content key is therefore also always out of ",
        "date, and so is everything downstream, which removes incremental ",
        "rebuilds from that whole branch of the graph."
    ),
    remediation: "Move the entry to `order_only_deps`, which sequences the work without forcing a rebuild.",
};

impl ManifestRule for PhonyDepOfFileTarget {
    fn meta(&self) -> &'static RuleMeta {
        &PHONY_DEP_OF_FILE_TARGET
    }

    fn check(&self, ctx: &ManifestContext<'_>, sink: &mut FindingSink<'_>) {
        let phony = phony_names(ctx.manifest);
        let provenance = Provenance::new(ctx.document, ctx.manifest);
        let file_targets = ctx
            .manifest
            .targets
            .iter()
            .enumerate()
            .filter(|(_, target)| !target.phony);
        for (index, target) in file_targets {
            report_entries(
                &Inspection {
                    target,
                    node: provenance.target(index),
                    offending: &phony,
                    kind: "phony target",
                },
                sink,
            );
        }
    }
}

/// Detects a content dependency on a directory-creating target.
pub struct DirectoryDepNotOrderOnly;

/// Metadata for [`DirectoryDepNotOrderOnly`].
static DIRECTORY_DEP_NOT_ORDER_ONLY: RuleMeta = RuleMeta {
    name: "directory-dep-not-order-only",
    category: Category::Caching,
    stage: Stage::Manifest,
    default_severity: DefaultSeverity::On(Severity::Warning),
    summary: "directory-creating target used as a content dependency",
    rationale: concat!(
        "A directory's modification time changes whenever any entry is created ",
        "or removed inside it. A target that depends on a directory through ",
        "`sources` or `deps` therefore rebuilds whenever a sibling output is ",
        "written, even though nothing it reads has changed."
    ),
    remediation: "Move the directory to `order_only_deps`, which guarantees it exists first without tracking its timestamp.",
};

impl ManifestRule for DirectoryDepNotOrderOnly {
    fn meta(&self) -> &'static RuleMeta {
        &DIRECTORY_DEP_NOT_ORDER_ONLY
    }

    fn check(&self, ctx: &ManifestContext<'_>, sink: &mut FindingSink<'_>) {
        let directories = directory_targets(ctx.manifest);
        let provenance = Provenance::new(ctx.document, ctx.manifest);
        for (index, target) in ctx.manifest.targets.iter().enumerate() {
            report_entries(
                &Inspection {
                    target,
                    node: provenance.target(index),
                    offending: &directories,
                    kind: "directory",
                },
                sink,
            );
        }
    }
}

/// One target under inspection, together with the paths that offend it.
struct Inspection<'a> {
    /// The expanded target being inspected.
    target: &'a Target,
    /// The target's authored node, when provenance resolved one.
    node: Option<&'a Node>,
    /// Paths whose appearance under a content key is a finding.
    offending: &'a [&'a str],
    /// What the offending paths are, for the diagnostic message.
    kind: &'static str,
}

/// Report every content-key entry of a target that names an offending path.
fn report_entries(inspection: &Inspection<'_>, sink: &mut FindingSink<'_>) {
    let matches = content_entries(inspection.target)
        .into_iter()
        .filter(|(_, entry)| inspection.offending.contains(entry));
    for (key, entry) in matches {
        sink.at_or_detached(
            resolve::entry_span(inspection.node, key, entry),
            label(inspection.target),
            format!(
                "depends on the {} `{entry}` through `{key}`",
                inspection.kind
            ),
        );
    }
}

/// Collect the names of every phony target and action.
fn phony_names(manifest: &NetsukeManifest) -> Vec<&str> {
    manifest
        .actions
        .iter()
        .chain(&manifest.targets)
        .filter(|target| target.phony)
        .flat_map(names)
        .collect()
}

/// Collect the names of every target whose recipe creates a directory.
///
/// Directory creation is recognized from the recipe's leading command, which
/// is how manifests express it: Netsuke has no directory target type, so
/// `mkdir` is the signal available.
fn directory_targets(manifest: &NetsukeManifest) -> Vec<&str> {
    let rules: BTreeMap<&str, &Recipe> = manifest
        .rules
        .iter()
        .map(|rule| (rule.name.as_str(), &rule.recipe))
        .collect();
    manifest
        .targets
        .iter()
        .filter(|target| creates_directory(&target.recipe, &rules))
        .flat_map(names)
        .collect()
}

/// Report whether a recipe's every command creates a directory.
///
/// Every command must be a `mkdir`, so a recipe that creates a directory and
/// then writes a real output is not mistaken for a directory target.
fn creates_directory(recipe: &Recipe, rules: &BTreeMap<&str, &Recipe>) -> bool {
    match recipe {
        Recipe::Command { command } => {
            let commands = command.to_string_vec();
            !commands.is_empty() && commands.iter().all(|text| is_mkdir(text))
        }
        Recipe::Script { script } => !script.trim().is_empty() && is_mkdir(script),
        Recipe::Rule { rule } => rule
            .to_string_vec()
            .iter()
            .filter_map(|name| rules.get(name.as_str()))
            .any(|referenced| creates_directory(referenced, rules)),
    }
}

/// Report whether every segment of `text` invokes `mkdir`.
fn is_mkdir(text: &str) -> bool {
    let segments = shellscan::segments(text);
    let mut words = segments
        .iter()
        .filter_map(|(_, segment)| shellscan::leading_word(segment))
        .map(|(_, word)| word.rsplit('/').next().unwrap_or(word))
        .peekable();
    words.peek().is_some() && words.all(|word| word == "mkdir")
}

/// Collect the content-key entries of one target, paired with their key.
fn content_entries(target: &Target) -> Vec<(&'static str, &str)> {
    [
        (CONTENT_KEYS[0], &target.sources),
        (CONTENT_KEYS[1], &target.deps),
    ]
    .into_iter()
    .flat_map(|(key, list)| {
        list_entries(list)
            .into_iter()
            .map(move |entry| (key, entry))
    })
    .collect()
}

/// Borrow the entries of a `StringOrList`.
fn list_entries(list: &StringOrList) -> Vec<&str> {
    match list {
        StringOrList::Empty => Vec::new(),
        StringOrList::String(value) => vec![value.as_str()],
        StringOrList::List(values) => values.iter().map(String::as_str).collect(),
    }
}

/// Collect a target's declared output names.
fn names(target: &Target) -> Vec<&str> {
    list_entries(&target.name)
}

/// Name a target for diagnostics that cannot resolve a span.
fn label(target: &Target) -> String {
    format!("target `{}`", names(target).first().copied().unwrap_or(""))
}

#[cfg(test)]
#[path = "caching_tests.rs"]
mod tests;
