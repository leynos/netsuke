//! Unit tests for Netsuke manifest AST deserialisation.

use netsuke::ast::*;
use rstest::rstest;

#[rstest]
fn parse_minimal_manifest() {
    let yaml = r#"
        netsuke_version: "1.0"
        targets:
          - name: hello
            recipe:
              kind: command
              command: "echo hi"
    "#;

    let manifest: NetsukeManifest = serde_yaml::from_str(yaml).expect("parse");

    assert_eq!(manifest.netsuke_version, "1.0");
    let first = manifest.targets.first().expect("target");
    if let StringOrList::String(name) = &first.name {
        assert_eq!(name, "hello");
    } else {
        panic!("target name should be String");
    }
}
