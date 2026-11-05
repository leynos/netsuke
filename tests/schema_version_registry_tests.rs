//! Contract tests tying the design registry to reader-side schema admission.

use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs_utf8::Dir};
use netsuke::manifest;
use rstest::rstest;

const REGISTRY_START: &str = "<!-- schema-registry:start -->";
const REGISTRY_END: &str = "<!-- schema-registry:end -->";
const EXPECTED_REGISTRY: &[(&str, &str, Option<u64>)] = &[
    ("core", "released", Some(0)),
    ("structured-commands", "reserved", Some(1)),
    ("named-shells", "reserved", Some(1)),
    ("includes", "reserved", None),
    ("bundles", "reserved", None),
    ("host-facts", "reserved", None),
    ("inputs", "reserved", None),
    ("contention", "reserved", None),
    ("states", "reserved", None),
    ("artefacts", "reserved", None),
    ("maturity", "reserved", None),
];

#[derive(Clone, Debug, Eq, PartialEq)]
struct RegistryRow {
    feature: String,
    status: String,
    minor: Option<u64>,
}

fn registry_rows(document: &str) -> Result<Vec<RegistryRow>> {
    let (_, after_start) = document
        .split_once(REGISTRY_START)
        .context("design document should mark the schema registry start")?;
    let (table, _) = after_start
        .split_once(REGISTRY_END)
        .context("design document should mark the schema registry end")?;
    table
        .lines()
        .filter(|line| line.trim_start().starts_with('|'))
        .filter_map(|line| {
            let cells: Vec<_> = line.split('|').map(str::trim).collect();
            if cells
                .get(1)
                .is_some_and(|cell| *cell == "Feature identifier" || cell.starts_with("---"))
            {
                return None;
            }
            Some((|| {
                ensure!(
                    cells.len() == 10,
                    "schema registry data row should have eight cells: {line}"
                );
                let feature_cell = cells
                    .get(1)
                    .copied()
                    .context("schema registry row should include a feature identifier")?;
                let status_cell = cells
                    .get(6)
                    .copied()
                    .context("schema registry row should include a status")?;
                let minor_cell = cells
                    .get(7)
                    .copied()
                    .context("schema registry row should include a schema minor")?;
                let feature = feature_cell.trim_matches('`').to_owned();
                let status = status_cell.to_owned();
                let minor = match minor_cell.trim_matches('`') {
                    "—" => None,
                    value => {
                        let (major, minor) = value
                            .split_once('.')
                            .with_context(|| format!("parse schema version for {feature}"))?;
                        ensure!(
                            major == "1",
                            "schema registry feature {feature} must use schema major 1"
                        );
                        Some(
                            minor
                                .parse::<u64>()
                                .with_context(|| format!("parse schema minor for {feature}"))?,
                        )
                    }
                };
                Ok(RegistryRow {
                    feature,
                    status,
                    minor,
                })
            })())
        })
        .collect()
}

fn validate_registry(rows: &[RegistryRow], supported_minor: u64) -> Result<()> {
    let admitted: Vec<_> = rows
        .iter()
        .filter(|row| matches!(row.status.as_str(), "admitted" | "released"))
        .filter_map(|row| row.minor)
        .collect();
    let highest = admitted
        .iter()
        .copied()
        .max()
        .context("registry should admit at least one schema minor")?;
    ensure!(
        highest == supported_minor,
        "highest admitted minor {highest} does not match reader minor {supported_minor}"
    );
    for row in rows.iter().filter(|row| row.status == "reserved") {
        if let Some(minor) = row.minor {
            ensure!(
                !admitted
                    .iter()
                    .any(|admitted_minor| *admitted_minor > minor),
                "reserved minor {minor} for {} interrupts the admitted range",
                row.feature
            );
        }
    }
    Ok(())
}

fn table_with_rows(rows: &str) -> String {
    format!(
        concat!(
            "{REGISTRY_START}\n",
            "| Feature identifier | Owner | Keys | Fields | Values | Status | Schema minor | Tag |\n",
            "| ------------------ | ----- | ---- | ------ | ------ | ------ | ------------ | --- |\n",
            "{rows}\n",
            "{REGISTRY_END}\n",
        ),
        REGISTRY_START = REGISTRY_START,
        REGISTRY_END = REGISTRY_END,
        rows = rows,
    )
}

fn manifest_for_minor(minor: u64) -> String {
    format!("netsuke_version: '1.{minor}.0'\ntargets:\n  - name: hello\n    command: echo hello\n")
}

#[rstest]
fn documented_registry_matches_reader_admission_boundary() -> Result<()> {
    let docs = Dir::open_ambient_dir("docs", ambient_authority())
        .context("open the documentation directory")?;
    let document = docs
        .read_to_string("netsuke-design.md")
        .context("read the schema version registry")?;
    let rows = registry_rows(&document)?;
    ensure!(
        rows.iter()
            .map(|row| (row.feature.as_str(), row.status.as_str(), row.minor))
            .eq(EXPECTED_REGISTRY.iter().copied()),
        "design document schema registry must retain all 11 documented feature entries"
    );
    validate_registry(&rows, 0)?;

    for row in &rows {
        let Some(minor) = row.minor else {
            continue;
        };
        let result = manifest::from_str(&manifest_for_minor(minor));
        match row.status.as_str() {
            "admitted" | "released" => ensure!(
                result.is_ok(),
                "reader rejected admitted schema minor {minor} for {}",
                row.feature
            ),
            "reserved" => ensure!(
                result.is_err(),
                "reader accepted reserved schema minor {minor} for {}",
                row.feature
            ),
            _ => ensure!(false, "unknown registry status `{}`", row.status),
        }
    }
    Ok(())
}

#[rstest]
fn registry_rejects_malformed_data_rows() -> Result<()> {
    let error = registry_rows(&table_with_rows(
        "| `core` | existing | core | none | none | released | 1.0 |",
    ))
    .expect_err("a data row missing its tag cell must be rejected");
    ensure!(
        error.to_string().contains("eight cells"),
        "unexpected malformed-row diagnostic: {error}"
    );
    Ok(())
}

#[rstest]
fn registry_rejects_an_admitted_minor_above_the_reader_maximum() -> Result<()> {
    let rows = registry_rows(&table_with_rows(
        "| `core` | existing | core | none | none | released | 1.0 | v1 |\n\
         | `future` | RFC | future | none | none | admitted | 1.1 | — |",
    ))?;
    let error = validate_registry(&rows, 0).expect_err("1.1 must exceed reader maximum 1.0");
    ensure!(
        error.to_string().contains("does not match reader"),
        "unexpected mismatch diagnostic: {error}"
    );
    Ok(())
}

#[rstest]
fn registry_rejects_a_reserved_minor_inside_the_admitted_range() -> Result<()> {
    let rows = registry_rows(&table_with_rows(
        "| `core` | existing | core | none | none | released | 1.0 | v1 |\n\
         | `gap` | RFC | gap | none | none | reserved | 1.1 | — |\n\
         | `later` | RFC | later | none | none | admitted | 1.2 | — |",
    ))?;
    let error = validate_registry(&rows, 2)
        .expect_err("a reserved 1.1 minor must interrupt admission of 1.2");
    ensure!(
        error.to_string().contains("interrupts the admitted range"),
        "unexpected contiguity diagnostic: {error}"
    );
    Ok(())
}
