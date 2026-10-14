//! Construct small synthetic validator inputs shared only by private tests.
//!
//! These constructors belong to the coverage suite's tests. They model parsed
//! data, never infer it from production parsers, and are not runtime APIs.

use super::{Namespace, Registration, Row, World, map, registries, roadmap, survey};
use std::collections::{BTreeMap, BTreeSet};

/// Construct one filter introduced by the survey.
pub(super) fn helper(name: &str) -> Row {
    Row {
        name: name.into(),
        namespace: Namespace::Filter,
    }
}

/// Construct a self-consistent survey containing one new pure filter.
pub(super) fn survey() -> survey::Survey {
    let accepted = BTreeMap::from([("helper".into(), helper("helper"))]);
    let mut denied: BTreeSet<_> = [
        "is_file",
        "is_dir",
        "is_link",
        "quote",
        "fileglob",
        "lookup",
        "win_dirname",
        "expanduser",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    denied.extend((0..63).map(|index| format!("denied{index}")));
    survey::Survey {
        accepted,
        new_rows: vec![helper("helper")],
        optioned: vec![],
        denied,
        sections: BTreeMap::new(),
        accept_rows: 1,
        defer_rows: 0,
        reject_rows: 0,
        stated_accept: 1,
        stated_defer: 0,
        reject_classes: [0; 3],
        new_filters: 1,
        new_tests: 0,
        optioned_total: 0,
        purity: (1, 0, 0),
        proposed: 1,
    }
}

/// Construct one new pure registry row.
pub(super) fn registry() -> registries::Registry {
    registries::Registry {
        file: "docs/rfcs/0013-child.md".into(),
        number: "0013".into(),
        rows: vec![registries::RegistryRow {
            helper: helper("helper"),
            registration: Registration::New,
            purity: registries::Purity::Pure,
        }],
    }
}

/// Construct eight groups, one written, with one registry and one scheduled helper.
pub(super) fn world() -> World {
    let rows = (0..8)
        .map(|index| map::MapRow {
            number: format!("{:04}", 13 + index),
            written: None,
            owns: if index == 0 {
                vec!["helper".into()]
            } else {
                vec![]
            },
            optioned: vec![],
            step: format!("6.{}", index + 2),
            is_written: index == 0,
        })
        .collect();
    World {
        survey: survey(),
        map: map::Map { rows },
        registries: vec![registry()],
        roadmap: roadmap::Steps {
            names: (2..10)
                .map(|step| {
                    (
                        format!("6.{step}"),
                        if step == 2 {
                            BTreeSet::from(["helper".into()])
                        } else {
                            BTreeSet::new()
                        },
                    )
                })
                .collect(),
            order: vec!["6.2".into()],
        },
        child_paths: BTreeMap::new(),
    }
}
