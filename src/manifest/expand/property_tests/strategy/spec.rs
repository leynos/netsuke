//! The authored-entry model an oracle reads.
//!
//! The model holds what the author wrote, in authoring order, and derives the
//! expected expanded shape from it. It deliberately contains no
//! `serde_json::Map` removal: the oracle must not share the operation under
//! test.

use super::condition::Condition;
use super::{Bindings, ForeachForm, Items, OVERRIDE_NAMES, Section};
use crate::manifest::{ManifestMap, ManifestValue};
use proptest::sample::Index;
use serde_json::Number;

/// Where the control block sits in the authored key sequence.
#[derive(Clone, Copy, Debug)]
pub(super) enum ControlLayout {
    /// A drawn position among the authored keys.
    Drawn(Index),
    /// Forced to the end, for the coverage test's negative control.
    Last,
}

impl ControlLayout {
    /// A drawn position among the authored keys.
    pub(super) const fn drawn(index: Index) -> Self {
        Self::Drawn(index)
    }

    /// Resolve the position the block is inserted at.
    fn position(self, base_len: usize) -> usize {
        match self {
            Self::Drawn(index) => index.index(base_len + 1),
            Self::Last => base_len,
        }
    }
}

/// The authored side of one entry: everything except its section.
#[derive(Clone, Debug)]
pub(in crate::manifest::expand::property_tests) struct BodySpec {
    /// Non-control fields in authoring order, duplicate names removed.
    pub(super) fields: Vec<(String, ManifestValue)>,
    /// Authored `vars` entries in authoring order, or `None` when absent.
    pub(super) vars: Option<Vec<(String, ManifestValue)>>,
    /// Where `vars` sits among the fields.
    pub(super) vars_position: Index,
    /// The `foreach` spelling and values, when the entry has one.
    pub(super) foreach: Option<(ForeachForm, Items)>,
    /// The condition and whether it is written in template form.
    pub(super) when: Option<(Condition, bool)>,
    /// Where the control keys sit among the authored keys.
    pub(super) control_layout: ControlLayout,
    /// The values the condition's free variable sees.
    pub(super) bindings: Bindings,
}

impl BodySpec {
    /// Report whether this entry iterates.
    pub(in crate::manifest::expand::property_tests) const fn is_foreach(&self) -> bool {
        self.foreach.is_some()
    }

    /// The authored key sequence with the control keys in place.
    fn authored_entries(&self) -> Vec<(String, ManifestValue)> {
        let mut entries: Vec<(String, ManifestValue)> = Vec::new();
        let vars_at = self.vars_position.index(self.fields.len() + 1);
        for (offset, (key, value)) in self.fields.iter().enumerate() {
            if offset == vars_at
                && let Some(vars) = &self.vars
            {
                entries.push(("vars".to_owned(), vars_object(vars)));
            }
            entries.push((key.clone(), value.clone()));
        }
        if vars_at == self.fields.len()
            && let Some(vars) = &self.vars
        {
            entries.push(("vars".to_owned(), vars_object(vars)));
        }

        let mut controls: Vec<(String, ManifestValue)> = Vec::new();
        if let Some((form, items)) = &self.foreach {
            let value = match form {
                ForeachForm::InlineArray => items.inline_array(),
                ForeachForm::ListLiteral => ManifestValue::String(items.list_literal()),
            };
            controls.push(("foreach".to_owned(), value));
        }
        if let Some((condition, template)) = self.when {
            let rendered = condition.render();
            let value = if template {
                format!("{{{{ {rendered} }}}}")
            } else {
                rendered
            };
            controls.push(("when".to_owned(), ManifestValue::String(value)));
        }

        let position = self.control_layout.position(entries.len());
        for (offset, control) in controls.into_iter().enumerate() {
            entries.insert(position + offset, control);
        }
        entries
    }

    /// The entry-level key sequence an expanded entry must carry.
    pub(in crate::manifest::expand::property_tests) fn predicted_keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = self
            .authored_entries()
            .into_iter()
            .map(|(key, _)| key)
            .filter(|key| key != "foreach" && key != "when")
            .collect();
        if self.is_foreach() && self.vars.is_none() {
            keys.push("vars".to_owned());
        }
        keys
    }

    /// The `vars` key sequence an expanded entry must carry.
    ///
    /// Returns `None` when the expanded entry carries no `vars` at all.
    pub(in crate::manifest::expand::property_tests) fn predicted_vars_keys(
        &self,
    ) -> Option<Vec<String>> {
        let mut keys: Vec<String> = self
            .vars
            .as_ref()
            .map(|vars| vars.iter().map(|(key, _)| key.clone()).collect())
            .unwrap_or_default();
        if self.is_foreach() {
            for injected in OVERRIDE_NAMES {
                if !keys.iter().any(|key| key == injected) {
                    keys.push(injected.to_owned());
                }
            }
        }
        (self.vars.is_some() || self.is_foreach()).then_some(keys)
    }

    /// The `vars` value an expanded entry must carry at one iteration.
    pub(in crate::manifest::expand::property_tests) fn predicted_vars(
        &self,
        iteration: Option<(ManifestValue, usize)>,
    ) -> Option<ManifestMap> {
        let mut map = self
            .vars
            .as_ref()
            .map(|vars| ManifestMap::from_iter(vars.iter().cloned()))
            .unwrap_or_default();
        let has_iteration = iteration.is_some();
        if let Some((item, index)) = iteration {
            map.insert("item".to_owned(), item);
            map.insert(
                "index".to_owned(),
                ManifestValue::Number(Number::from(index as u64)),
            );
        }
        (self.vars.is_some() || has_iteration).then_some(map)
    }

    /// Report whether the entry's condition holds at one iteration.
    pub(in crate::manifest::expand::property_tests) fn condition_holds(
        &self,
        item: Option<i64>,
        index: usize,
    ) -> bool {
        self.when
            .is_none_or(|(condition, _)| condition.holds(&self.bindings, item, index))
    }

    /// The authored fields, for the field-preservation obligation.
    pub(in crate::manifest::expand::property_tests) fn fields(&self) -> &[(String, ManifestValue)] {
        &self.fields
    }

    /// Report whether the authored entry carries a `vars` object at all.
    pub(in crate::manifest::expand::property_tests) const fn has_vars(&self) -> bool {
        self.vars.is_some()
    }

    /// Report whether the authored entry carries a condition.
    pub(in crate::manifest::expand::property_tests) const fn has_condition(&self) -> bool {
        self.when.is_some()
    }

    /// Report which `foreach` spelling the entry uses.
    pub(in crate::manifest::expand::property_tests) fn foreach_form(&self) -> Option<ForeachForm> {
        self.foreach.as_ref().map(|(form, _)| *form)
    }

    /// Report whether the condition is written in template form.
    pub(in crate::manifest::expand::property_tests) const fn condition_is_template(&self) -> bool {
        matches!(self.when, Some((_, true)))
    }

    /// Borrow the iteration values, when the entry iterates.
    pub(in crate::manifest::expand::property_tests) fn items(&self) -> Option<&Items> {
        self.foreach.as_ref().map(|(_, items)| items)
    }

    /// Look one authored field up by name.
    pub(in crate::manifest::expand::property_tests) fn field_value(
        &self,
        key: &str,
    ) -> Option<&ManifestValue> {
        self.fields
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value)
    }

    /// Borrow the condition, when the entry has one.
    pub(in crate::manifest::expand::property_tests) const fn condition(&self) -> Option<Condition> {
        match self.when {
            Some((condition, _)) => Some(condition),
            None => None,
        }
    }

    /// Report how many authored keys follow the control block.
    ///
    /// This is the class `swap_remove` corrupts when it is two or more.
    pub(in crate::manifest::expand::property_tests) fn following_key_count(&self) -> usize {
        let base = self.fields.len() + usize::from(self.vars.is_some());
        base - self.control_layout.position(base)
    }

    /// Drop the condition, keeping everything else as authored.
    pub(in crate::manifest::expand::property_tests) fn without_condition(mut self) -> Self {
        self.when = None;
        self
    }

    /// Force the control block to the end of the authored sequence.
    ///
    /// This exists for the coverage test's negative control: a strategy built
    /// on it can never produce a control key followed by another key, which is
    /// exactly the class the check must report as unreached.
    pub(in crate::manifest::expand::property_tests) fn with_control_last(mut self) -> Self {
        self.control_layout = ControlLayout::Last;
        self
    }

    /// Look one authored `vars` value up by name.
    pub(in crate::manifest::expand::property_tests) fn vars_value(
        &self,
        key: &str,
    ) -> Option<&ManifestValue> {
        self.vars
            .as_ref()
            .and_then(|vars| vars.iter().find(|(name, _)| name == key))
            .map(|(_, value)| value)
    }

    /// The authored `vars` keys, for collision checks.
    pub(in crate::manifest::expand::property_tests) fn vars_keys(&self) -> Vec<&str> {
        self.vars
            .as_ref()
            .map(|vars| vars.iter().map(|(key, _)| key.as_str()).collect())
            .unwrap_or_default()
    }
}

/// Build the object an authored `vars` list denotes.
fn vars_object(vars: &[(String, ManifestValue)]) -> ManifestValue {
    ManifestValue::Object(ManifestMap::from_iter(vars.iter().cloned()))
}

/// One authored entry, with its section.
#[derive(Clone, Debug)]
pub(in crate::manifest::expand::property_tests) struct EntrySpec {
    /// The section the entry is written in.
    pub(in crate::manifest::expand::property_tests) section: Section,
    /// The authored body.
    pub(in crate::manifest::expand::property_tests) body: BodySpec,
}

impl EntrySpec {
    /// Render the authored entry as the loader would receive it.
    pub(in crate::manifest::expand::property_tests) fn render(&self) -> ManifestValue {
        ManifestValue::Object(ManifestMap::from_iter(self.body.authored_entries()))
    }

    /// Render a document holding just this entry.
    pub(in crate::manifest::expand::property_tests) fn render_document(&self) -> ManifestValue {
        document_with(self.section.name(), &[self])
    }
}

/// A whole generated manifest: entries in each section.
#[derive(Clone, Debug)]
pub(in crate::manifest::expand::property_tests) struct DocumentSpec {
    /// Entries authored in `targets`.
    pub(in crate::manifest::expand::property_tests) targets: Vec<EntrySpec>,
    /// Entries authored in `actions`.
    pub(in crate::manifest::expand::property_tests) actions: Vec<EntrySpec>,
}

impl DocumentSpec {
    /// Render the document the loader would receive.
    pub(in crate::manifest::expand::property_tests) fn render(&self) -> ManifestValue {
        let mut document = ManifestMap::new();
        for section in [Section::Targets, Section::Actions] {
            let entries: Vec<ManifestValue> = self
                .entries(section)
                .iter()
                .map(EntrySpec::render)
                .collect();
            document.insert(section.name().to_owned(), ManifestValue::Array(entries));
        }
        ManifestValue::Object(document)
    }

    /// Borrow one section's entries.
    pub(in crate::manifest::expand::property_tests) fn entries(
        &self,
        section: Section,
    ) -> &[EntrySpec] {
        match section {
            Section::Targets => &self.targets,
            Section::Actions => &self.actions,
        }
    }

    /// Borrow every entry in both sections, in document order.
    pub(in crate::manifest::expand::property_tests) fn all_entries(&self) -> Vec<&EntrySpec> {
        self.targets.iter().chain(self.actions.iter()).collect()
    }
}

/// Build a document holding `entries` in one section.
fn document_with(section: &str, entries: &[&EntrySpec]) -> ManifestValue {
    let mut document = ManifestMap::new();
    document.insert(
        section.to_owned(),
        ManifestValue::Array(entries.iter().map(|entry| entry.render()).collect()),
    );
    ManifestValue::Object(document)
}
