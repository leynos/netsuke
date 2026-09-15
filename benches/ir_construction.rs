//! Benchmark lowering a large multi-output manifest into the canonical IR.

#![feature(test)]

extern crate test;

use netsuke::{ir::BuildGraph, manifest};
use test::{Bencher, black_box};

/// Build a manifest with many explicit outputs outside the timed closure.
fn multi_output_manifest() -> netsuke::ast::NetsukeManifest {
    let outputs = (0..4_096)
        .map(|index| format!("out/{index:04}"))
        .collect::<Vec<_>>()
        .join(", ");
    match manifest::from_str(&format!(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: [{outputs}]\n    command: echo {{{{ outs }}}}\n"
    )) {
        Ok(manifest) => manifest,
        Err(error) => panic!("build multi-output benchmark manifest: {error}"),
    }
}

/// Benchmark canonical IR construction for thousands of output aliases.
#[bench]
fn lowers_thousands_of_outputs_linearly(bencher: &mut Bencher) {
    let manifest = multi_output_manifest();
    bencher.iter(|| {
        let graph = match BuildGraph::from_manifest(black_box(&manifest)) {
            Ok(graph) => graph,
            Err(error) => panic!("lower multi-output benchmark manifest: {error}"),
        };
        black_box(graph);
    });
}
