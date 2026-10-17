//! Pure regressions for selecting direct Cargo build-dependency package IDs.

use super::{build_dependency_ids_from_metadata, build_dependency_package_ids, metadata_command};
use serde_json::{Value, json};
use std::{
    ffi::OsString,
    path::Path,
    process::{ExitStatus, Output},
};

/// Build the smallest Cargo metadata document needed by the resolver.
fn metadata(root_id: Option<&str>, nodes: Value) -> String {
    let resolve = serde_json::Map::from_iter([
        (String::from("root"), json!(root_id)),
        (String::from("nodes"), nodes),
    ]);
    serde_json::Value::Object(serde_json::Map::from_iter([(
        String::from("resolve"),
        serde_json::Value::Object(resolve),
    )]))
    .to_string()
}

/// Build a root or dependency package node with its direct dependency edges.
fn node(package_id: &str, dependencies: Value) -> Value {
    serde_json::Value::Object(serde_json::Map::from_iter([
        (String::from("id"), json!(package_id)),
        (String::from("deps"), dependencies),
    ]))
}

/// Build a dependency edge with the supplied package ID and dependency kinds.
fn edge(name: &str, package_id: Option<&str>, kinds: Value) -> Value {
    serde_json::Value::Object(serde_json::Map::from_iter([
        (String::from("name"), json!(name)),
        (String::from("pkg"), json!(package_id)),
        (String::from("dep_kinds"), kinds),
    ]))
}

/// Encode one Cargo dependency kind, including Cargo's null normal-kind form.
fn kind(value: Value) -> Value {
    serde_json::Value::Object(serde_json::Map::from_iter([
        (String::from("kind"), value),
        (String::from("target"), Value::Null),
    ]))
}

#[test]
fn metadata_command_preserves_cargo_program_and_query_flags() {
    let cargo_executable = Path::new("missing-cargo-for-command-builder-test");
    let manifest_path = Path::new("fixture/Cargo.toml");
    let command = metadata_command(cargo_executable, manifest_path);
    let arguments = command.get_args().map(OsString::from).collect::<Vec<_>>();

    assert_eq!(command.get_program(), cargo_executable.as_os_str());
    assert_eq!(
        arguments,
        vec![
            OsString::from("metadata"),
            OsString::from("--format-version=1"),
            OsString::from("--all-features"),
            OsString::from("--locked"),
            OsString::from("--manifest-path"),
            manifest_path.as_os_str().to_owned(),
        ],
    );
    assert!(command.get_envs().next().is_none());
}

#[test]
fn captured_successful_output_resolves_requested_build_dependencies() {
    let nodes = json!([
        node(
            "root-id",
            json!([edge(
                "camino",
                Some("camino-id"),
                json!([kind(json!("build"))])
            )]),
        ),
        node("camino-id", json!([])),
    ]);
    let output = Output {
        status: exit_status(true),
        stdout: metadata(Some("root-id"), nodes).into_bytes(),
        stderr: Vec::new(),
    };

    assert_eq!(
        build_dependency_package_ids(&output, &["camino"])
            .expect("successful captured Cargo metadata should be parsed"),
        vec![("camino", String::from("camino-id"))],
    );
}

#[test]
fn captured_failure_preserves_exit_status_and_stderr() {
    let output = Output {
        status: exit_status(false),
        stdout: b"not parsed after a Cargo failure".to_vec(),
        stderr: b"Cargo could not resolve the locked graph".to_vec(),
    };
    let status = output.status.to_string();
    let error = build_dependency_package_ids(&output, &["camino"])
        .expect_err("unsuccessful metadata commands must fail");

    assert!(error.to_string().contains(&status));
    assert!(
        error
            .to_string()
            .contains("Cargo could not resolve the locked graph")
    );
}

/// Construct a platform-specific exit status for captured-output tests.
#[cfg(unix)]
fn exit_status(success: bool) -> ExitStatus {
    use std::os::unix::process::ExitStatusExt;

    ExitStatus::from_raw(if success { 0 } else { 1 << 8 })
}

/// Construct a platform-specific exit status for captured-output tests.
#[cfg(windows)]
fn exit_status(success: bool) -> ExitStatus {
    use std::os::windows::process::ExitStatusExt;

    ExitStatus::from_raw(if success { 0 } else { 1 })
}

#[test]
fn resolver_selects_build_edge_and_preserves_requested_order() {
    let nodes = json!([
        node(
            "root-id",
            json!([
                edge("camino", Some("normal-camino"), json!([kind(Value::Null)])),
                edge(
                    "camino",
                    Some("build-camino"),
                    json!([kind(json!("build"))])
                ),
                edge("clap", Some("build-clap"), json!([kind(json!("build"))])),
                edge("other", Some("dev-only"), json!([kind(json!("dev"))])),
            ]),
        ),
        node("normal-camino", json!([])),
        node("build-camino", json!([])),
        node("build-clap", json!([])),
        node("dev-only", json!([])),
    ]);

    let selected =
        build_dependency_ids_from_metadata(&metadata(Some("root-id"), nodes), &["clap", "camino"])
            .expect("build edges should resolve by opaque package ID");

    assert_eq!(
        selected,
        vec![
            ("clap", String::from("build-clap")),
            ("camino", String::from("build-camino")),
        ],
    );
}

#[test]
fn resolver_deduplicates_repeated_edge_to_same_package_id() {
    let nodes = json!([
        node(
            "root-id",
            json!([
                edge("camino", Some("camino-id"), json!([kind(json!("build"))])),
                edge("camino", Some("camino-id"), json!([kind(json!("build"))])),
            ]),
        ),
        node("camino-id", json!([])),
    ]);

    assert_eq!(
        build_dependency_ids_from_metadata(&metadata(Some("root-id"), nodes), &["camino"],)
            .expect("repeated identical edges should identify one package"),
        vec![("camino", String::from("camino-id"))],
    );
}

#[test]
fn resolver_rejects_missing_or_ambiguous_build_dependency() {
    let normal_only = metadata(
        Some("root-id"),
        json!([
            node(
                "root-id",
                json!([edge(
                    "camino",
                    Some("normal-id"),
                    json!([kind(Value::Null)])
                )]),
            ),
            node("normal-id", json!([])),
        ]),
    );
    let missing = build_dependency_ids_from_metadata(&normal_only, &["camino"])
        .expect_err("normal dependencies must not satisfy a build dependency request");
    assert!(missing.to_string().contains("no build dependency"));

    let ambiguous = metadata(
        Some("root-id"),
        json!([
            node(
                "root-id",
                json!([
                    edge("camino", Some("camino-a"), json!([kind(json!("build"))])),
                    edge("camino", Some("camino-b"), json!([kind(json!("build"))])),
                ]),
            ),
            node("camino-a", json!([])),
            node("camino-b", json!([])),
        ]),
    );
    let error = build_dependency_ids_from_metadata(&ambiguous, &["camino"])
        .expect_err("distinct build package IDs must be rejected");
    assert!(error.to_string().contains("ambiguous build dependency"));
}

#[rstest::rstest]
#[case(None, json!([]), "resolve.root")]
#[case(Some("missing-root"), json!([]), "resolve root node")]
#[case(Some("root-id"), json!([json!({"id":"root-id"}), json!({"id":"root-id"})]), "duplicate resolve node ID")]
fn resolver_rejects_missing_or_duplicate_root_nodes(
    #[case] root_id: Option<&str>,
    #[case] nodes: Value,
    #[case] expected_error: &str,
) {
    let error = build_dependency_ids_from_metadata(&metadata(root_id, nodes), &[])
        .expect_err("invalid root-node identity must fail");

    assert!(error.to_string().contains(expected_error));
}

#[rstest::rstest]
#[case(edge("camino", None, json!([kind(json!("build"))])), "root deps[].pkg")]
#[case(edge("camino", Some("missing-node"), json!([kind(json!("build"))])), "unknown package ID")]
#[case(edge("camino", Some("camino-id"), json!([json!({"kind":42})])), "dep_kinds[0].kind")]
fn resolver_rejects_malformed_or_unresolved_edges(
    #[case] dependency: Value,
    #[case] expected_error: &str,
) {
    let nodes = json!([
        node("root-id", json!([dependency])),
        node("camino-id", json!([])),
    ]);
    let error = build_dependency_ids_from_metadata(&metadata(Some("root-id"), nodes), &["camino"])
        .expect_err("malformed or unresolved dependency edges must fail");

    assert!(error.to_string().contains(expected_error));
}
