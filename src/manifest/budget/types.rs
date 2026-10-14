//! Define manifest-budget limits and safe diagnostic vocabulary.

use anyhow::{Result, ensure};

/// Default maximum `MiniJinja` instructions reserved for one evaluation.
pub const DEFAULT_EVALUATION_FUEL: u64 = 1_000_000;
/// Default maximum bytes produced by one rendered manifest value.
pub const DEFAULT_RENDERED_VALUE_BYTES: usize = 1_048_576;
/// Default maximum rendered bytes produced by one manifest.
pub const DEFAULT_RENDERED_MANIFEST_BYTES: usize = 16_777_216;
/// Default maximum source bytes consumed by templates and macro imports.
pub const DEFAULT_SOURCE_BYTES: usize = 4_194_304;
/// Default maximum items consumed by one `foreach` expression.
pub const DEFAULT_FOREACH_CARDINALITY: usize = 10_000;
/// Default maximum targets and actions emitted by expansion.
pub const DEFAULT_EXPANDED_ENTRIES: usize = 50_000;
/// Default maximum `MiniJinja` instructions reserved across one manifest.
pub const DEFAULT_MANIFEST_FUEL: u64 = 100_000_000;

/// Configure the resource ceilings applied to a single manifest evaluation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ManifestBudgetLimits {
    /// Caps fuel reserved for one render or expression evaluation.
    pub evaluation_fuel: u64,
    /// Caps the bytes emitted by one rendered string value.
    pub rendered_value_bytes: usize,
    /// Caps all rendered bytes emitted while loading one manifest.
    pub rendered_manifest_bytes: usize,
    /// Caps all template and generated macro-import source bytes.
    pub source_bytes: usize,
    /// Caps the values consumed from one `foreach` iterator.
    pub foreach_cardinality: usize,
    /// Caps target and action entries emitted by expansion.
    pub expanded_entries: usize,
    /// Caps fuel reserved across all manifest evaluations.
    pub manifest_fuel: u64,
}

impl Default for ManifestBudgetLimits {
    /// Return the safe production resource ceilings.
    fn default() -> Self {
        Self {
            evaluation_fuel: DEFAULT_EVALUATION_FUEL,
            rendered_value_bytes: DEFAULT_RENDERED_VALUE_BYTES,
            rendered_manifest_bytes: DEFAULT_RENDERED_MANIFEST_BYTES,
            source_bytes: DEFAULT_SOURCE_BYTES,
            foreach_cardinality: DEFAULT_FOREACH_CARDINALITY,
            expanded_entries: DEFAULT_EXPANDED_ENTRIES,
            manifest_fuel: DEFAULT_MANIFEST_FUEL,
        }
    }
}

impl ManifestBudgetLimits {
    /// Validate that every configured resource ceiling is positive.
    ///
    /// # Errors
    ///
    /// Returns an error when an operator configures a zero ceiling.
    pub fn validate(self) -> Result<Self> {
        ensure!(
            self.evaluation_fuel > 0,
            "manifest evaluation fuel must be positive"
        );
        ensure!(
            self.rendered_value_bytes > 0,
            "manifest rendered value bytes must be positive"
        );
        ensure!(
            self.rendered_manifest_bytes > 0,
            "manifest rendered bytes must be positive"
        );
        ensure!(
            self.source_bytes > 0,
            "manifest source bytes must be positive"
        );
        ensure!(
            self.foreach_cardinality > 0,
            "manifest foreach cardinality must be positive"
        );
        ensure!(
            self.expanded_entries > 0,
            "manifest expanded entries must be positive"
        );
        ensure!(self.manifest_fuel > 0, "manifest fuel must be positive");
        Ok(self)
    }
}

/// Name the closed-vocabulary stage at which manifest work is charged.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ManifestBudgetStage {
    /// Render a normal manifest field.
    Render,
    /// Consume a `foreach` iterator.
    Foreach,
    /// Evaluate a `when` condition.
    When,
    /// Invoke a manifest macro.
    Macro,
    /// Clone an expansion entry.
    ExpansionAggregate,
    /// Account aggregate rendered output.
    ByteAggregate,
    /// Parse template and macro-import source.
    Source,
    /// Execute `MiniJinja` instructions.
    Fuel,
}

impl ManifestBudgetStage {
    /// Return every permitted telemetry stage in its stable vocabulary.
    pub(crate) const fn all() -> [Self; 8] {
        [
            Self::Render,
            Self::Foreach,
            Self::When,
            Self::Macro,
            Self::ExpansionAggregate,
            Self::ByteAggregate,
            Self::Source,
            Self::Fuel,
        ]
    }

    /// Return the stable telemetry and diagnostic stage label.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Render => "render",
            Self::Foreach => "foreach",
            Self::When => "when",
            Self::Macro => "macro",
            Self::ExpansionAggregate => "expansion_aggregate",
            Self::ByteAggregate => "byte_aggregate",
            Self::Source => "source",
            Self::Fuel => "fuel",
        }
    }
}

/// Name the resource counter exhausted by an operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ManifestBudgetKind {
    /// Counts `MiniJinja` instructions.
    Fuel,
    /// Counts bytes emitted by one rendered value.
    ValueBytes,
    /// Counts aggregate rendered bytes.
    RenderedBytes,
    /// Counts template and generated import source bytes.
    SourceBytes,
    /// Counts values consumed by one `foreach` expression.
    ForeachCardinality,
    /// Counts target and action entries emitted by expansion.
    ExpandedEntries,
}

impl ManifestBudgetKind {
    /// Return the stable telemetry budget label.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Fuel => "fuel",
            Self::ValueBytes => "value_bytes",
            Self::RenderedBytes => "rendered_bytes",
            Self::SourceBytes => "source_bytes",
            Self::ForeachCardinality => "foreach_cardinality",
            Self::ExpandedEntries => "expanded_entries",
        }
    }
}

/// Describe one deterministic resource-budget exhaustion.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("manifest resource budget exhausted at {stage:?} (limit {limit})")]
pub(crate) struct ManifestBudgetExhaustion {
    /// Identifies the exhausted counter.
    pub(crate) kind: ManifestBudgetKind,
    /// Identifies the evaluation stage that attempted the charge.
    pub(crate) stage: ManifestBudgetStage,
    /// Supplies the configured limit without manifest-controlled data.
    pub(crate) limit: u64,
}

#[cfg(test)]
mod tests {
    //! Test manifest-budget limit validation at its lower boundary.

    use super::*;
    use rstest::{fixture, rstest};

    /// Supply limits at the smallest valid value.
    #[fixture]
    fn one_unit_limits() -> ManifestBudgetLimits {
        ManifestBudgetLimits {
            evaluation_fuel: 1,
            rendered_value_bytes: 1,
            rendered_manifest_bytes: 1,
            source_bytes: 1,
            foreach_cardinality: 1,
            expanded_entries: 1,
            manifest_fuel: 1,
        }
    }

    /// Replace one named limit with zero for recognised boundary cases.
    fn with_zero_limit(
        mut limits: ManifestBudgetLimits,
        field: &str,
    ) -> Option<ManifestBudgetLimits> {
        match field {
            "evaluation_fuel" => limits.evaluation_fuel = 0,
            "rendered_value_bytes" => limits.rendered_value_bytes = 0,
            "rendered_manifest_bytes" => limits.rendered_manifest_bytes = 0,
            "source_bytes" => limits.source_bytes = 0,
            "foreach_cardinality" => limits.foreach_cardinality = 0,
            "expanded_entries" => limits.expanded_entries = 0,
            "manifest_fuel" => limits.manifest_fuel = 0,
            _ => return None,
        }
        Some(limits)
    }

    #[rstest]
    #[case::evaluation_fuel("evaluation_fuel", "manifest evaluation fuel")]
    #[case::rendered_value_bytes("rendered_value_bytes", "manifest rendered value bytes")]
    #[case::rendered_manifest_bytes("rendered_manifest_bytes", "manifest rendered bytes")]
    #[case::source_bytes("source_bytes", "manifest source bytes")]
    #[case::foreach_cardinality("foreach_cardinality", "manifest foreach cardinality")]
    #[case::expanded_entries("expanded_entries", "manifest expanded entries")]
    #[case::manifest_fuel("manifest_fuel", "manifest fuel")]
    fn zero_limit_is_rejected(
        one_unit_limits: ManifestBudgetLimits,
        #[case] field: &str,
        #[case] expected: &str,
    ) {
        let error = with_zero_limit(one_unit_limits, field)
            .expect("test fields must name a manifest budget limit")
            .validate()
            .expect_err("zero manifest budget limit must be rejected");
        assert!(
            error.to_string().contains(expected),
            "{field} should identify its validation error: {error:#}"
        );
    }

    #[rstest]
    fn one_unit_limits_are_valid(one_unit_limits: ManifestBudgetLimits) -> Result<()> {
        let _ = one_unit_limits.validate()?;
        Ok(())
    }
}
