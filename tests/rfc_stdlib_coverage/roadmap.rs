//! Reading the roadmap's phase 6 capability steps.
//!
//! Roadmap 6.1.1 settles that delivery is tracked by roadmap checkboxes rather
//! than by issues, so the roadmap is the only place a reviewer can see which
//! capability is still outstanding. That makes the link from a child RFC to its
//! step load-bearing: a helper with no roadmap task has no channel through which
//! it can be scheduled, and a step whose bullets name no helper has nothing to
//! tick off.
//!
//! Steps 6.1 and 6.10 onwards are outside the capability set. Step 6.1 is this
//! split itself, and 6.10 decides the deferred candidates, which by construction
//! must not appear in a registry.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use super::{ROADMAP, Repo, Section, backticked};

/// The first capability step's heading text, with the `### ` prefix removed.
const FIRST_STEP: &str = "6.2.";

/// The heading text that ends the capability range, `### ` prefix removed.
const LAST_STEP: &str = "6.10.";

/// The roadmap's capability steps, keyed by their number.
pub(super) struct Steps {
    /// Backticked names per step number, in document order.
    pub(super) names: BTreeMap<String, BTreeSet<String>>,
    /// The step number each heading carried, in document order.
    pub(super) order: Vec<String>,
}

impl Steps {
    /// The names named anywhere in the capability steps.
    pub(super) fn all_names(&self) -> BTreeSet<String> {
        self.names.values().flatten().cloned().collect()
    }

    /// The backticked names a step contains, or an error naming the known steps.
    pub(super) fn names_for(&self, step: &str) -> Result<&BTreeSet<String>> {
        self.names.get(step).with_context(|| {
            format!(
                "roadmap step {step} does not exist; the capability steps are {:?}",
                self.order
            )
        })
    }

    /// The subset of `names` that appears in no capability step at all.
    pub(super) fn unscheduled<'a>(
        &self,
        names: impl IntoIterator<Item = &'a String>,
    ) -> Vec<String> {
        let scheduled = self.all_names();
        names
            .into_iter()
            .filter(|name| !scheduled.contains(*name))
            .cloned()
            .collect()
    }

    /// The subset of `names` absent from `step`.
    pub(super) fn missing_from_step<'a>(
        &self,
        step: &str,
        names: impl IntoIterator<Item = &'a String>,
    ) -> Result<Vec<String>> {
        let present = self.names_for(step)?;
        Ok(names
            .into_iter()
            .filter(|name| !present.contains(*name))
            .cloned()
            .collect())
    }
}

/// Read the roadmap's capability steps.
pub(super) fn parse(repo: &Repo) -> Result<Steps> {
    let text = repo.read(ROADMAP)?;
    let document = Section::whole(&text, ROADMAP);
    let section_6 = document
        .subsection("## 6. Template standard-library expansion")
        .context("docs/roadmap.md has no section 6")?;

    let mut names: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut current: Option<String> = None;
    let mut in_range = false;

    for line in &section_6.lines {
        if let Some(heading) = line.strip_prefix("### ") {
            let number = heading.split('.').take(2).collect::<Vec<_>>().join(".");
            if heading.starts_with(FIRST_STEP) {
                in_range = true;
            } else if heading.starts_with(LAST_STEP) {
                in_range = false;
            }
            if in_range {
                ensure!(
                    number.starts_with("6."),
                    "roadmap step heading {heading:?} is not numbered under phase 6"
                );
                order.push(number.clone());
                names.entry(number.clone()).or_default();
                current = Some(number);
            } else {
                current = None;
            }
            continue;
        }
        let Some(step) = &current else {
            continue;
        };
        names
            .entry(step.clone())
            .or_default()
            .extend(backticked(line));
    }

    ensure!(
        !order.is_empty(),
        "docs/roadmap.md has no capability steps starting {FIRST_STEP}"
    );
    ensure!(
        names.len() == 8,
        "docs/roadmap.md yields {} capability steps between {FIRST_STEP} and {LAST_STEP}; \
         expected eight, one per child RFC 0013 to 0020",
        names.len()
    );
    ensure!(
        names.values().any(|step| !step.is_empty()),
        "no roadmap capability step names a backticked helper, so the task-bullet parse matched \
         nothing and every helper would look unscheduled"
    );
    Ok(Steps { names, order })
}
