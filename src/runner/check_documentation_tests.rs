//! Contract tests for runner-owned rule-reference publication URLs.

use crate::lint::catalogue;

use super::rule_documentation_url;

/// Point every registered rule at its published reference section.
#[test]
fn documentation_urls_point_into_the_reference() {
    for meta in catalogue() {
        let url = rule_documentation_url(meta.name);
        assert_eq!(
            url,
            format!(
                "https://github.com/leynos/netsuke/blob/main/docs/netsuke-linter-rules.md#{}",
                meta.name
            ),
            "`{}` should link to its published reference section",
            meta.name,
        );
    }
}
