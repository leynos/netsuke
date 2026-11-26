//! The authored-entry model an oracle reads.
//!
//! The model holds what the author wrote, in authoring order, and derives the
//! expected expanded shape from it. It deliberately contains no
//! `serde_json::Map` removal: the oracle must not share the operation under
//! test.

use super::condition::Condition;
use super::layout::{ControlLayout, ControlOrder};
use super::{Bindings, ForeachForm, Items, OVERRIDE_NAMES, Section};
use crate::manifest::{ManifestMap, ManifestValue};
use proptest::sample::Index;
use serde_json::Number;

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
    /// The slot the first-written control key takes.
    pub(super) control_layout: ControlLayout,
    /// The slot the second-written control key takes.
    pub(super) second_control_layout: ControlLayout,
    /// Which control key is written first when both are present.
    pub(super) control_order: ControlOrder,
    /// The values the condition's free variable sees.
    pub(super) bindings: Bindings,
}

impl BodySpec {
    /// Report whether this entry iterates.
    pub(in crate::manifest::expand::property_tests) const fn is_foreach(&self) -> bool {
        self.foreach.is_some()
    }

    /// The authored key sequence with the control keys in place.
    ///
    /// Each control key has its own independently drawn slot. The key written
    /// first takes the lower slot; on a tie it is inserted after the other, so
    /// it still ends up first. The recorded order and the observed sequence
    /// therefore cannot disagree.
    fn authored_entries(&self) -> Vec<(String, ManifestValue)> {
        let mut entries = self.fields_with_vars();
        let base = entries.len();
        let mut controls = self.control_entries().into_iter();
        let Some(first) = controls.next() else {
            return entries;
        };
        let first_slot = self.control_layout.position(base);
        let Some(second) = controls.next() else {
            entries.insert(first_slot.min(base), first);
            return entries;
        };
        let second_slot = self.second_control_layout.position(base);
        let (low_slot, high_slot) = (first_slot.min(second_slot), first_slot.max(second_slot));
        let (low_key, high_key) = if first_slot <= second_slot {
            (first, second)
        } else {
            (second, first)
        };
        entries.insert(high_slot.min(entries.len()), high_key);
        entries.insert(low_slot.min(entries.len()), low_key);
        entries
    }

    /// The authored fields with the `vars` object spliced in at its position.
    fn fields_with_vars(&self) -> Vec<(String, ManifestValue)> {
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
        entries
    }

    /// The control keys this entry carries, in the authored order.
    fn control_entries(&self) -> Vec<(String, ManifestValue)> {
        let mut controls: Vec<(String, ManifestValue)> =
            self.foreach_control().into_iter().collect();
        if let Some(when) = self.when_control() {
            if matches!(self.control_order, ControlOrder::WhenFirst) {
                controls.insert(0, when);
            } else {
                controls.push(when);
            }
        }
        controls
    }

    /// Build the `foreach` control entry, when the entry iterates.
    fn foreach_control(&self) -> Option<(String, ManifestValue)> {
        let (form, items) = self.foreach.as_ref()?;
        let value = match form {
            ForeachForm::InlineArray => items.inline_array(),
            ForeachForm::ListLiteral => ManifestValue::String(items.list_literal()),
        };
        Some(("foreach".to_owned(), value))
    }

    /// Build the `when` control entry, when the entry has a condition.
    fn when_control(&self) -> Option<(String, ManifestValue)> {
        let (condition, template) = self.when?;
        Some(("when".to_owned(), rendered_condition(condition, template)))
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
        let mut keys: Vec<String> = self.vars.as_ref().map_or_else(Vec::new, |vars| {
            vars.iter().map(|(key, _)| key.clone()).collect()
        });
        if self.is_foreach() {
            for injected in OVERRIDE_NAMES {
                append_missing(&mut keys, injected);
            }
        }
        (self.vars.is_some() || self.is_foreach()).then_some(keys)
    }

    /// The `vars` value an expanded entry must carry at one iteration.
    pub(in crate::manifest::expand::property_tests) fn predicted_vars(
        &self,
        iteration: Option<&(ManifestValue, usize)>,
    ) -> Option<ManifestMap> {
        let mut map = self.vars.as_ref().map_or_else(ManifestMap::new, |vars| {
            ManifestMap::from_iter(vars.iter().cloned())
        });
        let has_iteration = iteration.is_some();
        if let Some((item, index)) = iteration {
            map.insert("item".to_owned(), item.clone());
            map.insert(
                "index".to_owned(),
                ManifestValue::Number(Number::from(u64::try_from(*index).unwrap_or(u64::MAX))),
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
        let first = self.control_layout.position(base);
        let earliest = match self.control_entries().len() {
            0 | 1 => first,
            _ => first.min(self.second_control_layout.position(base)),
        };
        base.saturating_sub(earliest)
    }

    /// Report which control key the entry writes first.
    pub(in crate::manifest::expand::property_tests) const fn control_order(&self) -> ControlOrder {
        self.control_order
    }

    /// Report whether the entry's two control keys share one slot.
    ///
    /// Derived from the drawn slots, not stored beside them.
    pub(in crate::manifest::expand::property_tests) fn control_same_slot(&self) -> bool {
        if self.control_entries().len() < 2 {
            return true;
        }
        let base = self.fields.len() + usize::from(self.vars.is_some());
        self.control_layout.position(base) == self.second_control_layout.position(base)
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
        self.second_control_layout = ControlLayout::Last;
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
        self.vars.as_ref().map_or_else(Vec::new, |vars| {
            vars.iter().map(|(key, _)| key.as_str()).collect()
        })
    }
}

/// Render one condition as the entry's `when` value.
fn rendered_condition(condition: Condition, template: bool) -> ManifestValue {
    let rendered = condition.render();
    if template {
        ManifestValue::String(format!("{{{{ {rendered} }}}}"))
    } else {
        ManifestValue::String(rendered)
    }
}

/// Append `key` unless the sequence already contains it.
fn append_missing(keys: &mut Vec<String>, key: &str) {
    if !keys.iter().any(|candidate| candidate == key) {
        keys.push(key.to_owned());
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
