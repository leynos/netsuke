//! Which graph paths may reach a generated bundle, and how they are escaped.
//!
//! The sibling tests in [`super`] cover staging *structure* — which edges get
//! gates, how many statements they contribute, and what the sidecars reveal.
//! This module covers the orthogonal question of what a path may contain. Each
//! graph position is swept because the checks live at different layers: some
//! reject during string generation, some only in bundle generation, and the
//! reserved-namespace check runs over every edge path field and over the
//! graph's default targets.

use super::{graph_with_edge, parallel_edge, serial_edge};
use crate::ir::BuildEdge;
use crate::ninja_gen::NinjaGenError;
use crate::ninja_gen::dyndep::generate_bundle;
use anyhow::{Context, Result, ensure};
use camino::Utf8PathBuf;
use rstest::rstest;

/// One position in an edge that accepts a graph path.
#[derive(Clone, Copy)]
enum EdgePathField {
    ExplicitOutput,
    ImplicitOutput,
    Input,
    ImplicitDependency,
    OrderOnlyDependency,
}

/// A path position that may be a graph-wide default rather than an edge field.
#[derive(Clone, Copy)]
enum GraphPathField {
    Edge(EdgePathField),
    DefaultTarget,
}

/// Replace one edge path field with `path`, leaving the others intact.
fn set_edge_path(edge: &mut BuildEdge, field: EdgePathField, path: &str) {
    let paths = vec![Utf8PathBuf::from(path)];
    match field {
        EdgePathField::ExplicitOutput => edge.explicit_outputs = paths,
        EdgePathField::ImplicitOutput => edge.implicit_outputs = paths,
        EdgePathField::Input => edge.inputs = paths,
        EdgePathField::ImplicitDependency => edge.implicit_deps = paths,
        EdgePathField::OrderOnlyDependency => edge.order_only_deps = paths,
    }
}

/// Build a graph carrying `path` in `field`.
///
/// Replacing the explicit output would leave the edge unnamed, so the original
/// `all` alias is retained ahead of the replacement.
fn graph_with_path(field: GraphPathField, path: &str) -> crate::ir::BuildGraph {
    let mut edge = parallel_edge("all", &["dep"]);
    if let GraphPathField::Edge(edge_field) = field {
        set_edge_path(&mut edge, edge_field, path);
        if matches!(edge_field, EdgePathField::ExplicitOutput) {
            edge.explicit_outputs.insert(0, Utf8PathBuf::from("all"));
        }
    }
    let mut graph = graph_with_edge(edge);
    if matches!(field, GraphPathField::DefaultTarget) {
        graph.default_targets = vec![Utf8PathBuf::from(path)];
    }
    graph
}

#[rstest]
fn unsupported_control_characters_are_rejected_in_every_path_field(
    #[values(
        GraphPathField::Edge(EdgePathField::ExplicitOutput),
        GraphPathField::Edge(EdgePathField::ImplicitOutput),
        GraphPathField::Edge(EdgePathField::Input),
        GraphPathField::Edge(EdgePathField::ImplicitDependency),
        GraphPathField::Edge(EdgePathField::OrderOnlyDependency),
        GraphPathField::DefaultTarget
    )]
    field: GraphPathField,
    #[values('\0', '\t', '\r', '\n')] character: char,
) -> Result<()> {
    let path = format!("invalid{character}path");
    let graph = graph_with_path(field, &path);
    let error = generate_bundle(&graph)
        .err()
        .context("unsupported control character must be rejected")?;
    ensure!(
        matches!(
            error,
            NinjaGenError::UnsupportedPathCharacter {
                character: actual,
                ..
            } if actual == character
        ),
        "expected UnsupportedPathCharacter for {character:?}, got {error:?}"
    );
    Ok(())
}

#[rstest]
#[case::dollar("path$money", '$')]
#[case::colon("path:colon", ':')]
fn ninja_metacharacters_are_rejected_in_every_path_field(
    #[values(
        GraphPathField::Edge(EdgePathField::ExplicitOutput),
        GraphPathField::Edge(EdgePathField::ImplicitOutput),
        GraphPathField::Edge(EdgePathField::Input),
        GraphPathField::Edge(EdgePathField::ImplicitDependency),
        GraphPathField::Edge(EdgePathField::OrderOnlyDependency),
        GraphPathField::DefaultTarget
    )]
    field: GraphPathField,
    #[case] path: &str,
    #[case] character: char,
) -> Result<()> {
    let graph = graph_with_path(field, path);
    let generated_error = crate::ninja_gen::generate(&graph)
        .err()
        .context("Ninja metacharacter must be rejected during string generation")?;
    let bundle_error = generate_bundle(&graph)
        .err()
        .context("Ninja metacharacter must be rejected during bundle generation")?;

    ensure!(
        matches!(
            generated_error,
            NinjaGenError::UnsupportedPathCharacter {
                character: actual,
                ..
            } if actual == character
        ),
        "string generation must reject {character:?}, got {generated_error:?}"
    );
    ensure!(
        matches!(
            bundle_error,
            NinjaGenError::UnsupportedPathCharacter {
                character: actual,
                ..
            } if actual == character
        ),
        "bundle generation must reject {character:?}, got {bundle_error:?}"
    );
    Ok(())
}

/// Render whitespace in every graph path field with Ninja's literal-space escape.
#[rstest]
fn whitespace_paths_are_escaped_in_every_path_field(
    #[values(
        GraphPathField::Edge(EdgePathField::ExplicitOutput),
        GraphPathField::Edge(EdgePathField::ImplicitOutput),
        GraphPathField::Edge(EdgePathField::Input),
        GraphPathField::Edge(EdgePathField::ImplicitDependency),
        GraphPathField::Edge(EdgePathField::OrderOnlyDependency),
        GraphPathField::DefaultTarget
    )]
    field: GraphPathField,
) -> Result<()> {
    let graph = graph_with_path(field, "path with space");
    let generated = crate::ninja_gen::generate(&graph)?;
    let bundle = generate_bundle(&graph)?;
    ensure!(generated.contains("path$ with$ space"));
    ensure!(bundle.build_file().contains("path$ with$ space"));
    Ok(())
}

#[rstest]
#[case::explicit_output(EdgePathField::ExplicitOutput)]
#[case::implicit_output(EdgePathField::ImplicitOutput)]
#[case::input(EdgePathField::Input)]
#[case::implicit_dependency(EdgePathField::ImplicitDependency)]
#[case::order_only_dependency(EdgePathField::OrderOnlyDependency)]
fn reserved_namespace_is_rejected(
    #[case] field: EdgePathField,
    // Every namespace the generator writes into, including the two fixed names
    // the phony lowering emits, so a manifest cannot forge or shadow them.
    #[values(
        ".netsuke/serial/x",
        ".netsuke/dyndep/x",
        ".netsuke/phony/x",
        ".netsuke/phony/force",
        ".netsuke/phony/edge-0123456789abcdef"
    )]
    path: &str,
) -> Result<()> {
    let mut edge = parallel_edge("all", &["dep"]);
    set_edge_path(&mut edge, field, path);
    if edge.explicit_outputs == [Utf8PathBuf::from(path)] {
        edge.explicit_outputs.insert(0, Utf8PathBuf::from("all"));
    }
    let graph = graph_with_edge(edge);
    let err = generate_bundle(&graph)
        .err()
        .context("reserved path must be rejected")?;
    ensure!(
        matches!(err, NinjaGenError::ReservedOutputPath { .. }),
        "expected ReservedOutputPath, got {err:?}"
    );
    Ok(())
}

#[rstest]
#[case::exact(".netsuke/phony/force")]
#[case::child(".netsuke/phony/edge-0123456789abcdef")]
#[case::serial(".netsuke/serial/x")]
#[case::dyndep(".netsuke/dyndep/x")]
#[case::empty_segment(".netsuke//phony/force")]
#[case::dot_prefix("./.netsuke/phony/force")]
#[case::dot_segment(".netsuke/./phony/force")]
#[case::parent_round_trip("out/../.netsuke/phony/force")]
fn reserved_default_target_is_rejected(#[case] path: &str) -> Result<()> {
    let mut graph = graph_with_edge(parallel_edge("all", &["dep"]));
    graph.default_targets = vec![Utf8PathBuf::from(path)];

    let err = generate_bundle(&graph)
        .err()
        .context("a reserved default target must be rejected")?;
    ensure!(
        matches!(err, NinjaGenError::ReservedOutputPath { .. }),
        "expected ReservedOutputPath for {path}, got {err:?}"
    );
    Ok(())
}

/// A backslash is an ordinary filename character on Unix.
///
/// Normalizing through the platform path type would rewrite this spelling's
/// separators on Windows; the comparison must stay component-based so a Unix
/// name containing literal backslashes is not mistaken for the namespace.
#[cfg(unix)]
#[test]
fn a_literal_backslash_does_not_name_a_reserved_namespace() -> Result<()> {
    let mut graph = graph_with_edge(parallel_edge("all", &["dep"]));
    graph.default_targets = vec![Utf8PathBuf::from(".netsuke\\phony\\force")];

    generate_bundle(&graph).context("a literal backslash must not reach generated state")?;
    Ok(())
}

#[test]
fn unreserved_default_target_is_accepted() -> Result<()> {
    let mut graph = graph_with_edge(parallel_edge("all", &["dep"]));
    graph.default_targets = vec![Utf8PathBuf::from("./out/all")];

    generate_bundle(&graph)?;
    Ok(())
}

#[test]
fn similarly_prefixed_namespace_is_accepted() -> Result<()> {
    let mut edge = parallel_edge("all", &["dep"]);
    edge.implicit_outputs = vec![Utf8PathBuf::from(".netsuke-extra/x")];
    generate_bundle(&graph_with_edge(edge))?;
    Ok(())
}

#[test]
fn pipe_in_path_is_rejected_before_generation() -> Result<()> {
    let graph = graph_with_edge(serial_edge("all", &["unsupported|dependency", "test"]));
    let err = generate_bundle(&graph)
        .err()
        .context("pipe path must be rejected")?;
    ensure!(
        matches!(
            err,
            NinjaGenError::UnsupportedPathCharacter { character: '|', .. }
        ),
        "expected UnsupportedPathCharacter, got {err:?}"
    );
    Ok(())
}
