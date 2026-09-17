//! Translate domain budget failures at the manifest evaluation boundary.

use super::{budget::ManifestBudgetExhaustion, jinja_macros::telemetry};
use crate::localization::{self, LocalizedMessage, keys};
use minijinja::{Error, ErrorKind};

/// Evaluate a manifest and record budget failures only for full loading.
pub(super) fn from_str_named(
    yaml: &str,
    parse: super::ManifestParse<'_>,
    on_stage: &mut Option<&mut dyn FnMut(super::ManifestLoadStage)>,
) -> anyhow::Result<crate::ast::NetsukeManifest> {
    let is_query = matches!(
        parse.stdlib_registration,
        Some(super::StdlibRegistration::ManifestQuery)
    );
    let result = super::evaluate_manifest(yaml, parse, on_stage);
    if !is_query && let Err(error) = &result {
        record_exhaustion(error);
    }
    result
}

/// Adapt budget failures without adding effects to domain accounting.
pub(crate) trait BudgetErrorExt {
    /// Preserve typed exhaustion beneath a localized template-engine error.
    fn into_error(self, kind: ErrorKind) -> Error;
}

impl BudgetErrorExt for ManifestBudgetExhaustion {
    fn into_error(self, kind: ErrorKind) -> Error {
        Error::new(kind, exhaustion_message(&self).to_string()).with_source(self)
    }
}

/// Recover a bounded diagnostic without exposing enclosing template contexts.
pub(crate) fn budget_exhaustion_message(error: &anyhow::Error) -> Option<LocalizedMessage> {
    error
        .chain()
        .find_map(|source| source.downcast_ref::<ManifestBudgetExhaustion>())
        .map(exhaustion_message)
}

/// Describe exhaustion using only fixed stage vocabulary and its numeric limit.
fn exhaustion_message(exhaustion: &ManifestBudgetExhaustion) -> LocalizedMessage {
    localization::message(keys::MANIFEST_BUDGET_EXCEEDED)
        .with_arg("stage", exhaustion.stage.as_str())
        .with_arg("limit", exhaustion.limit)
}

/// Record the first domain exhaustion once at the full-manifest load boundary.
fn record_exhaustion(error: &anyhow::Error) {
    if let Some(exhaustion) = error
        .chain()
        .find_map(|source| source.downcast_ref::<ManifestBudgetExhaustion>())
    {
        telemetry::record_budget_exhaustion(exhaustion.stage.as_str(), exhaustion.kind.as_str());
    }
}

#[cfg(test)]
mod tests {
    //! Verify pure translation without secret-bearing error contexts.

    use super::{BudgetErrorExt, budget_exhaustion_message};
    use crate::manifest::budget::{
        ManifestBudgetExhaustion, ManifestBudgetKind, ManifestBudgetStage,
    };
    use metrics_util::debugging::DebuggingRecorder;

    #[test]
    fn translation_is_pure_and_discards_secret_context() {
        let recorder = DebuggingRecorder::new();
        let snapshotter = recorder.snapshotter();
        let message = metrics::with_local_recorder(&recorder, || {
            let exhaustion = ManifestBudgetExhaustion {
                kind: ManifestBudgetKind::ValueBytes,
                stage: ManifestBudgetStage::Render,
                limit: 16,
            };
            let error =
                anyhow::Error::new(exhaustion.into_error(minijinja::ErrorKind::WriteFailure))
                    .context("secret template context");
            budget_exhaustion_message(&error).expect("typed exhaustion must be recognized")
        });
        assert!(!message.to_string().contains("secret"));
        assert_eq!(snapshotter.snapshot().into_vec().len(), 0);
    }
}
