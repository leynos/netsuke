//! Every corpus manifest loads identically in both modes, or refuses by name.
//!
//! The property run covers manifests built to be pure. This one covers the
//! manifests the repository already keeps, including the ones that reach the
//! network, run a command, read the filesystem, look up a host tool, or consult
//! the clock: loading each with the same injected reader and the same fixed
//! clock must either produce the same result in the build and test surfaces, or
//! the test load must fail with a typed [`TestModeRefusal`] whose operation is
//! one the inventory withholds. It must never succeed with a *different*
//! manifest, because that would mean the test surface silently changed what the
//! author asked for rather than saying it cannot do it.
//!
//! "Same result" includes the failing case: two loads that fail with identical
//! error text are equal, because the test surface changed nothing the loader
//! would have reported. Two failures with *different* text are a difference
//! unless the test one is a refusal, which is the whole point of the refusal
//! carrying its operation as data.
//!
//! The run also asserts its own breadth. At least one file must come out equal,
//! and at least one refusal must be seen from each family of withheld
//! capability, so a surface that stopped refusing — or a corpus that stopped
//! exercising it — cannot pass by comparing nothing.
//!
//! The clock family is observed separately. Both modes are given the same fixed
//! clock, so `now()` is *admitted* under test and the corpus cannot produce a
//! clock refusal; [`ambient_clock_refusal`] loads the clock fixture once more
//! with the default clock, which is the configuration in which the refusal
//! exists at all.

use std::{collections::BTreeSet, error::Error as _, sync::Arc};

use anyhow::{Context as _, Result, bail, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::Error as MiniJinjaError;
use netsuke::{
    ast::NetsukeManifest,
    manifest::{
        EnvAccessPolicy, EnvReadError, EnvReader, ManifestEnvironment, ManifestLoadOptions,
        ManifestName, TestSurface, from_str_with_options,
    },
    stdlib::{StdlibConfig, TestModeRefusal, fixed_clock},
};
use time::OffsetDateTime;
use time::macros::datetime;

use super::fixtures::fixed_clock_config;

/// The instant both surfaces are configured with, so `now()` agrees.
const FIXED_INSTANT: OffsetDateTime = datetime!(2026-06-08 12:00:00 UTC);

/// The directories whose manifests make up the corpus.
const CORPUS_ROOTS: [&str; 2] = ["tests/data", "tests/data/manifest_load_options"];

/// The clock fixture, loaded again with the default clock for the clock family.
const CLOCK_FIXTURE: &str = "tests/data/manifest_load_options/clock_now.yml";

/// A family of withheld host capability.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Family {
    /// Reaching the network.
    Network,
    /// Spawning a process.
    Process,
    /// Touching the filesystem.
    Filesystem,
    /// Looking a tool up on the host.
    HostLookup,
    /// Consulting the ambient clock.
    Clock,
}

impl Family {
    /// Every family a refusal must be observed from.
    const ALL: [Self; 5] = [
        Self::Network,
        Self::Process,
        Self::Filesystem,
        Self::HostLookup,
        Self::Clock,
    ];

    /// Return the name used in a failure message.
    const fn label(self) -> &'static str {
        match self {
            Self::Network => "network",
            Self::Process => "process",
            Self::Filesystem => "filesystem",
            Self::HostLookup => "host lookup",
            Self::Clock => "clock",
        }
    }
}

/// The withheld operations, with the capability family each belongs to.
///
/// The integration crate cannot read the crate-internal `SURFACE` table, so
/// this one restates the classification. That is deliberate rather than
/// unfortunate: an operation refused without a family here makes this test
/// fail, which is the reminder to classify it. `env` is absent because the test
/// surface admits it. `now` is present even though the fixed-clock corpus run
/// admits it, because the ambient-clock probe withholds it.
const OPERATION_FAMILIES: &[(&str, Family)] = &[
    ("fetch", Family::Network),
    ("shell", Family::Process),
    ("grep", Family::Process),
    ("glob", Family::Filesystem),
    ("contents", Family::Filesystem),
    ("realpath", Family::Filesystem),
    ("expanduser", Family::Filesystem),
    ("size", Family::Filesystem),
    ("linecount", Family::Filesystem),
    ("hash", Family::Filesystem),
    ("digest", Family::Filesystem),
    ("dir", Family::Filesystem),
    ("file", Family::Filesystem),
    ("symlink", Family::Filesystem),
    ("pipe", Family::Filesystem),
    ("block_device", Family::Filesystem),
    ("char_device", Family::Filesystem),
    ("device", Family::Filesystem),
    ("which", Family::HostLookup),
    ("command_available", Family::HostLookup),
    ("now", Family::Clock),
];

/// Return the family a refused operation belongs to.
fn family_of(operation: &str) -> Option<Family> {
    OPERATION_FAMILIES
        .iter()
        .find(|(name, _)| *name == operation)
        .map(|(_, family)| *family)
}

/// Return the repository root, resolved at compile time.
fn repo_root() -> Utf8PathBuf {
    Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Return every corpus manifest path, repository-relative and deduplicated.
///
/// The second root is nested inside the first, so a plain concatenation would
/// visit those manifests twice. A set makes the walk's breadth independent of
/// how the roots happen to be laid out.
fn corpus_files() -> Result<BTreeSet<Utf8PathBuf>> {
    let root = Dir::open_ambient_dir(repo_root(), ambient_authority())
        .context("open the repository root")?;
    let mut files = BTreeSet::new();
    for directory in CORPUS_ROOTS {
        collect_manifests(&root, Utf8Path::new(directory), &mut files)
            .with_context(|| format!("walk `{directory}`"))?;
    }
    ensure!(!files.is_empty(), "the corpus walk found no manifests");
    Ok(files)
}

/// Append every YAML manifest beneath `directory` to `files`.
fn collect_manifests(
    root: &Dir,
    directory: &Utf8Path,
    files: &mut BTreeSet<Utf8PathBuf>,
) -> Result<()> {
    for entry in root
        .read_dir(directory)
        .with_context(|| format!("read `{directory}`"))?
    {
        let resolved = entry.with_context(|| format!("read an entry of `{directory}`"))?;
        let name = resolved
            .file_name()
            .with_context(|| format!("read an entry name in `{directory}`"))?;
        let path = directory.join(&name);
        if resolved.file_type()?.is_dir() {
            collect_manifests(root, &path, files)?;
        } else if matches!(path.extension(), Some("yml" | "yaml")) {
            files.insert(path);
        }
    }
    Ok(())
}

/// Read one corpus manifest.
fn read_manifest(path: &Utf8Path) -> Result<String> {
    let root = Dir::open_ambient_dir(repo_root(), ambient_authority())
        .context("open the repository root")?;
    root.read_to_string(path.as_str())
        .with_context(|| format!("read `{path}`"))
}

/// Return a reader that answers one name and refuses the rest.
fn reader() -> EnvReader {
    Arc::new(|name: &str| {
        if name == "NETSUKE_TEST_ENV" {
            Ok("corpus".to_owned())
        } else {
            Err(EnvReadError::NotPresent)
        }
    })
}

/// Load `yaml` through the build registration with `config`.
fn load_full(yaml: &str, config: &StdlibConfig) -> Result<NetsukeManifest> {
    let reader = reader();
    let environment = ManifestEnvironment::new(&reader, EnvAccessPolicy::default());
    from_str_with_options(
        yaml,
        &ManifestName::new("Netsukefile"),
        ManifestLoadOptions::full(config.clone(), environment),
    )
}

/// Load `yaml` through the test registration with `config`.
fn load_test(yaml: &str, config: &StdlibConfig) -> Result<NetsukeManifest> {
    let surface = TestSurface::new(config.clone()).with_env_reader(reader());
    from_str_with_options(
        yaml,
        &ManifestName::new("Netsukefile"),
        ManifestLoadOptions::test(surface),
    )
}

/// Return the operation named by a test-mode refusal in an error chain.
fn refusal_operation(error: &anyhow::Error) -> Option<&str> {
    error.chain().find_map(|cause| {
        let jinja = cause.downcast_ref::<MiniJinjaError>()?;
        jinja
            .source()?
            .downcast_ref::<TestModeRefusal>()
            .map(TestModeRefusal::operation)
    })
}

/// Render a manifest as canonical JSON for comparison.
fn canonical(manifest: &NetsukeManifest) -> Result<String> {
    serde_json_canonicalizer::to_string(manifest).context("serialize a loaded manifest")
}

/// Compare one manifest's outcome under both surfaces.
///
/// Returns the refused operation when the test load refused, and `None` when
/// the two outcomes were equal.
fn compare(text: &str, config: &StdlibConfig) -> Result<Option<String>> {
    let build_outcome = load_full(text, config);
    let test_outcome = load_test(text, config);
    match (&build_outcome, &test_outcome) {
        (Ok(build_manifest), Ok(test_manifest)) => {
            let build_json = canonical(build_manifest)?;
            let test_json = canonical(test_manifest)?;
            ensure!(
                build_json == test_json,
                "both surfaces loaded, but produced different manifests"
            );
            Ok(None)
        }
        (Ok(_), Err(error)) => {
            let operation = refusal_operation(error).with_context(|| {
                format!("the test load failed without a typed refusal: {error:#}")
            })?;
            Ok(Some(operation.to_owned()))
        }
        (Err(error), Ok(_)) => {
            bail!("the test load succeeded where the build load failed: {error:#}")
        }
        (Err(build_error), Err(test_error)) => {
            if let Some(operation) = refusal_operation(test_error) {
                return Ok(Some(operation.to_owned()));
            }
            let build_text = format!("{build_error:#}");
            let test_text = format!("{test_error:#}");
            ensure!(
                build_text == test_text,
                "both surfaces failed, but differently:\nBuild: {build_text}\nTest: {test_text}"
            );
            Ok(None)
        }
    }
}

/// Observe the clock refusal the fixed-clock corpus run cannot produce.
fn ambient_clock_refusal() -> Result<(String, String)> {
    let text = read_manifest(Utf8Path::new(CLOCK_FIXTURE))?;
    let surface = TestSurface::new(StdlibConfig::from_current_dir()?);
    let error = from_str_with_options(
        &text,
        &ManifestName::new("Netsukefile"),
        ManifestLoadOptions::test(surface),
    )
    .expect_err("the ambient clock must refuse under test");
    let operation = refusal_operation(&error)
        .with_context(|| format!("the clock fixture failed without a typed refusal: {error:#}"))?;
    Ok((CLOCK_FIXTURE.to_owned(), operation.to_owned()))
}

/// Every corpus manifest is equal across surfaces or refuses by name.
#[test]
fn corpus_loads_identically_or_refuses() -> Result<()> {
    let config = fixed_clock_config()?.with_clock(fixed_clock(FIXED_INSTANT));
    let mut equal = Vec::new();
    let mut refusals = vec![ambient_clock_refusal()?];
    let mut failures = Vec::new();

    for path in corpus_files()? {
        let text = match read_manifest(&path) {
            Ok(text) => text,
            Err(error) => {
                failures.push(format!("{path}: {error:#}"));
                continue;
            }
        };
        match compare(&text, &config) {
            Ok(None) => equal.push(path),
            Ok(Some(operation)) => refusals.push((path.to_string(), operation)),
            Err(error) => failures.push(format!("{path}: {error:#}")),
        }
    }

    ensure!(
        failures.is_empty(),
        "{} of the corpus disagreed between surfaces:\n{}",
        failures.len(),
        failures.join("\n")
    );
    ensure!(
        !equal.is_empty(),
        "no corpus manifest loaded identically in both surfaces; the comparison is vacuous"
    );
    assert_every_family_refuses(&refusals)?;
    Ok(())
}

/// Assert the run saw a refusal from every capability family.
fn assert_every_family_refuses(refusals: &[(String, String)]) -> Result<()> {
    let mut unclassified = Vec::new();
    let mut seen = BTreeSet::new();
    for (path, operation) in refusals {
        match family_of(operation) {
            Some(family) => {
                seen.insert(family);
            }
            None => unclassified.push(format!("{path}: `{operation}`")),
        }
    }
    ensure!(
        unclassified.is_empty(),
        "refused operations outside the classified inventory:\n{}",
        unclassified.join("\n")
    );
    let missing: Vec<&str> = Family::ALL
        .into_iter()
        .filter(|family| !seen.contains(family))
        .map(Family::label)
        .collect();
    ensure!(
        missing.is_empty(),
        "no corpus manifest refused from these families: {missing:?}; refusals seen: {refusals:?}"
    );
    Ok(())
}
