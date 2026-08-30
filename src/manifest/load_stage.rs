//! Manifest-loading pipeline stage definitions.

/// Stages in the manifest-loading sub-pipeline.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ManifestLoadStage {
    /// Read raw manifest content from the filesystem.
    ManifestIngestion,
    /// Parse raw YAML into a `serde_json::Value` tree.
    InitialYamlParsing,
    /// Expand `foreach` and `when` template directives.
    TemplateExpansion,
    /// Deserialize and render string fields into typed manifest data.
    FinalRendering,
}
