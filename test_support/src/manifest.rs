//! Helpers for constructing manifest fixtures in tests.

/// Prefix the provided manifest body with the standard Netsuke version header.
pub fn manifest_yaml(body: &str) -> String {
    format!("netsuke_version: 1.0.0\n{body}")
}
