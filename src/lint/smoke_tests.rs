//! Temporary smoke check over the repository's own example manifests.

use crate::lint::test_support::lint;

#[test]
fn examples_report_their_known_defects() {
    for path in [
        "examples/basic_c.yml",
        "examples/photo_edit.yml",
        "examples/visual_design.yml",
        "examples/website.yml",
        "examples/writing.yml",
        "examples/hello-world/Netsukefile",
    ] {
        let text = std::fs::read_to_string(path).expect("example should be readable");
        let outcome = lint(&text);
        println!("=== {path} ===");
        for finding in &outcome.findings {
            println!(
                "  [{}] {} :: {}",
                finding.severity,
                finding.meta.name,
                finding.display_message()
            );
        }
    }
}
