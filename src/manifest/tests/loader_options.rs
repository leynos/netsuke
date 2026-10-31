//! Exhaustive decision table for the manifest loader's registration modes.
//!
//! `StdlibRegistration` is the single place that decides a load's host-facing
//! capabilities (C5), so the decision is asserted directly here rather than
//! inferred from rendered output. The table is exhaustive over the variants
//! that exist, which is why a bounded model checker would add nothing.
//!
//! The `Test` row asserts the deny-by-default surface: `env()` is real over an
//! injected reader, `glob()` refuses, and the mode neither renders in query
//! mode, records normal-load telemetry, classifies query refusals, nor grants
//! overlays today. The overlay grant lands with EP-M4's machinery.

use rstest::rstest;

use crate::manifest::loader_options::HostHelperDisposition;
use crate::manifest::{
    EnvAccessPolicy, ManifestBudgetLimits, ManifestEnvironment, ManifestLoadOptions, ManifestName,
    StdlibRegistration, TestSurface, from_str_with_options, process_env_reader,
};
use crate::stdlib::{RestrictedSurface, StdlibConfig};

/// A manifest that reads and acts on nothing, so every mode can load it.
const PURE_MANIFEST: &str = concat!(
    "netsuke_version: \"1.0.0\"\n",
    "targets:\n",
    "  - name: pure\n",
    "    description: \"joined {{ ['a', 'b'] | join('-') }}\"\n",
    "    command: \"echo pure\"\n",
);

/// One registration mode under test.
#[derive(Clone, Copy, Debug)]
enum Row {
    AmbientFull,
    Full,
    ManifestQuery,
    Test,
}

/// The six outputs `StdlibRegistration` decides for one mode.
#[derive(Clone, Copy, Debug)]
struct Expected {
    env: HostHelperDisposition,
    glob: HostHelperDisposition,
    /// The four boolean decisions, each paired with the label the assertion
    /// reports, so one table row covers them without four near-identical
    /// assertions.
    flags: [(&'static str, bool); 4],
}

impl Row {
    /// Build the registration this row names.
    fn registration(self) -> StdlibRegistration {
        match self {
            Self::AmbientFull => StdlibRegistration::AmbientFull,
            Self::Full => StdlibRegistration::Full(Box::new(
                StdlibConfig::from_current_dir().expect("open the test workspace"),
            )),
            Self::ManifestQuery => StdlibRegistration::ManifestQuery,
            Self::Test => StdlibRegistration::Test(Box::new(TestSurface::new(
                StdlibConfig::from_current_dir().expect("open the test workspace"),
            ))),
        }
    }

    /// Return the decisions this row must produce.
    const fn expected(self) -> Expected {
        match self {
            Self::AmbientFull | Self::Full => Expected {
                env: HostHelperDisposition::Real,
                glob: HostHelperDisposition::Real,
                flags: [
                    ("render mode", false),
                    ("telemetry gate", true),
                    ("refusal classifier gate", false),
                    ("overlay grant", false),
                ],
            },
            Self::ManifestQuery => Expected {
                env: HostHelperDisposition::Refused(RestrictedSurface::ManifestQuery),
                glob: HostHelperDisposition::Refused(RestrictedSurface::ManifestQuery),
                flags: [
                    ("render mode", true),
                    ("telemetry gate", false),
                    ("refusal classifier gate", true),
                    ("overlay grant", false),
                ],
            },
            Self::Test => Expected {
                env: HostHelperDisposition::Real,
                glob: HostHelperDisposition::Refused(RestrictedSurface::Test),
                flags: [
                    ("render mode", false),
                    ("telemetry gate", false),
                    ("refusal classifier gate", false),
                    ("overlay grant", true),
                ],
            },
        }
    }
}

#[rstest]
#[case(Row::AmbientFull)]
#[case(Row::Full)]
#[case(Row::ManifestQuery)]
#[case(Row::Test)]
fn registration_decides_each_capability(#[case] row: Row) {
    let registration = row.registration();
    let expected = row.expected();

    assert_eq!(
        registration.env_disposition(),
        expected.env,
        "env() disposition for {row:?}"
    );
    assert_eq!(
        registration.glob_disposition(),
        expected.glob,
        "glob() disposition for {row:?}"
    );
    assert_eq!(
        [
            ("render mode", registration.uses_query_rendering()),
            (
                "telemetry gate",
                registration.records_expansion_and_budget_telemetry()
            ),
            (
                "refusal classifier gate",
                registration.classifies_query_refusals()
            ),
            ("overlay grant", registration.accepts_overlays()),
        ],
        expected.flags,
        "boolean decisions for {row:?}"
    );
}

#[test]
fn from_str_with_options_loads_a_full_manifest() {
    let reader = process_env_reader();
    let environment = ManifestEnvironment::new(&reader, EnvAccessPolicy::default());
    let config = StdlibConfig::from_current_dir().expect("open the test workspace");
    let options = ManifestLoadOptions::full(config, environment);

    let manifest = from_str_with_options(PURE_MANIFEST, &ManifestName::new("Netsukefile"), options)
        .expect("load the pure manifest");

    assert_eq!(manifest.targets.len(), 1);
    let target = manifest.targets.first().expect("one target was loaded");
    assert_eq!(target.name.to_string_vec(), vec!["pure".to_owned()]);
}

#[test]
fn builders_carry_the_budget_limits_and_stage_observer() {
    let reader = process_env_reader();
    let environment = ManifestEnvironment::new(&reader, EnvAccessPolicy::default());
    let mut stages = Vec::new();
    let mut observer = |stage| stages.push(stage);
    let config = StdlibConfig::from_current_dir().expect("open the test workspace");
    let options = ManifestLoadOptions::full(config, environment)
        .with_budget_limits(ManifestBudgetLimits::default())
        .with_stage_observer(&mut observer);

    from_str_with_options(PURE_MANIFEST, &ManifestName::new("Netsukefile"), options)
        .expect("load the pure manifest");

    assert_eq!(
        stages,
        vec![
            crate::manifest::ManifestLoadStage::InitialYamlParsing,
            crate::manifest::ManifestLoadStage::TemplateExpansion,
            crate::manifest::ManifestLoadStage::FinalRendering,
        ]
    );
}
