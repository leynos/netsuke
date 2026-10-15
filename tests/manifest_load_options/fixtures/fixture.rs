//! The characterization corpus and the inputs each fixture needs.
//!
//! Every fixture is a file under `tests/data/manifest_load_options/`. Holding
//! them all as files is a decision, not a default: a path entry point loads a
//! manifest from a workspace, and its error text names the manifest by the path
//! it was given, so the only way a path row can pin that text — and the only
//! way the `[TMP]` filter in [`super::settings`] has anything to filter — is
//! for the fixture to have a path. An inline fixture would leave both
//! unexercised.
//!
//! Each fixture earns its place: one is an ordinary load, one reads the
//! environment, one is not valid YAML, one renders differently under the build
//! and query surfaces, one consults the wall clock, two act on the host, and
//! one resolves a `glob()` pattern against its own directory.
//!
//! # Applicability
//!
//! What a fixture forces on its loader is recorded on [`Case`] rather than
//! inferred by each row, and [`Case::is_loadable`] is the single place the
//! rules are combined. Four constraints do the work:
//!
//! - A fixture with siblings needs a workspace, because its `glob()` has
//!   nothing to resolve against otherwise.
//! - An environment- or clock-reading fixture needs the matching seam, or a
//!   loader that answers the helper with a refusal. A refusal is deterministic;
//!   the process environment and the wall clock are not.
//! - A fixture that acts on the host is loadable *only* where the surface
//!   refuses the helper. That is what keeps a characterization run from running
//!   a command or reaching the network, and it is why those fixtures are
//!   observed through a refusal rather than an effect.

use std::sync::Arc;
use time::OffsetDateTime;
use time::macros::datetime;

use super::settings::repo_root;
use netsuke::manifest::{
    EnvAccessPolicy, EnvReadError, EnvReader, ManifestBudgetLimits, ManifestEnvironment,
};
use netsuke::stdlib::{StdlibConfig, fixed_clock};

/// The instant every clock-carrying row pins, so `now()` is repeatable.
pub const FIXED_INSTANT: OffsetDateTime = datetime!(2026-06-08 12:00:00 UTC);

/// Return the characterization corpus.
pub const fn cases() -> &'static [Case] {
    &[
        Case {
            label: "pure",
            path: concat!("tests/data/manifest_load_options/", "pure.yml"),
            siblings: &[],
            hazards: Hazards::NONE,
        },
        Case {
            label: "reads_env",
            path: concat!("tests/data/manifest_load_options/", "reads_env.yml"),
            siblings: &[],
            hazards: Hazards {
                reads_environment: true,
                ..Hazards::NONE
            },
        },
        Case {
            label: "malformed_yaml",
            path: concat!("tests/data/manifest_load_options/", "malformed.yml"),
            siblings: &[],
            hazards: Hazards::NONE,
        },
        Case {
            label: "boolean_interpolation",
            path: concat!(
                "tests/data/manifest_load_options/",
                "boolean_interpolation.yml"
            ),
            siblings: &[],
            hazards: Hazards::NONE,
        },
        Case {
            label: "clock_now",
            path: concat!("tests/data/manifest_load_options/", "clock_now.yml"),
            siblings: &[],
            hazards: Hazards {
                consults_clock: true,
                ..Hazards::NONE
            },
        },
        Case {
            label: "uses_fetch",
            path: concat!("tests/data/manifest_load_options/", "uses_fetch.yml"),
            siblings: &[],
            hazards: Hazards {
                acts_on_host: true,
                ..Hazards::NONE
            },
        },
        Case {
            label: "uses_shell",
            path: concat!("tests/data/manifest_load_options/", "uses_shell.yml"),
            siblings: &[],
            hazards: Hazards {
                acts_on_host: true,
                ..Hazards::NONE
            },
        },
        Case {
            label: "glob_corpus",
            path: "tests/data/glob.yml",
            siblings: &["tests/data/glob_files/a.txt", "tests/data/glob_files/b.txt"],
            hazards: Hazards::NONE,
        },
    ]
}

/// One manifest in the characterization corpus.
#[derive(Clone, Copy, Debug)]
pub struct Case {
    /// Unique, filesystem-safe label; every row reports it beside the result.
    pub label: &'static str,
    /// The manifest, named repository-relative.
    pub path: &'static str,
    /// Supporting files copied into the same directory before a path load.
    ///
    /// A fixture with siblings is only meaningful from a workspace: the loader
    /// anchors the manifest at a temporary directory, so a sibling left behind
    /// in the repository would not be found and the row would pass over an
    /// empty match set.
    pub siblings: &'static [&'static str],
    /// What the manifest's content forces on the loader.
    pub hazards: Hazards,
}

impl Case {
    /// Read the manifest text, so a string entry point can load it.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixture cannot be read.
    pub fn manifest_text(self) -> anyhow::Result<String> {
        test_support::fs::read_to_string(repo_root().join(self.path)).map_err(Into::into)
    }

    /// Report whether the fixture needs a workspace to resolve its content.
    pub const fn requires_workspace(self) -> bool {
        !self.siblings.is_empty()
    }

    /// Report whether `capabilities` describes a loader that can load this
    /// fixture repeatably.
    ///
    /// Each rule pairs something the fixture needs with something the loader
    /// offers, and they are stated once here rather than restated by every row.
    pub const fn is_loadable(self, capabilities: Capabilities) -> bool {
        if !capabilities.from_workspace && self.requires_workspace() {
            return false;
        }
        if self.hazards.reads_environment && !capabilities.resolves_environment_hazard() {
            return false;
        }
        if self.hazards.consults_clock && !capabilities.resolves_clock_hazard() {
            return false;
        }
        if self.hazards.acts_on_host && !capabilities.refusing_helpers {
            return false;
        }
        true
    }
}

/// What a fixture's content forces on the loader.
#[derive(Clone, Copy, Debug, Default)]
pub struct Hazards {
    /// The manifest acts on the host: it calls `shell()`, `grep()`, or
    /// `fetch()` in a field the loader renders.
    pub acts_on_host: bool,
    /// The manifest calls `env()`, so it needs a loader with an injected reader
    /// rather than the process environment.
    pub reads_environment: bool,
    /// The manifest calls `now()`, so it needs a loader that can inject a
    /// clock.
    pub consults_clock: bool,
}

impl Hazards {
    /// A manifest with no hazards at all.
    pub const NONE: Self = Self {
        acts_on_host: false,
        reads_environment: false,
        consults_clock: false,
    };
}

/// What a loader is able to offer the corpus.
///
/// The two injected seams are held as one field rather than two sibling
/// Booleans because they answer one question — which boundaries does the caller
/// supply? — and because grouping them keeps the struct within Clippy's
/// four-Boolean ceiling without bending the rules that read it. They are not
/// forced to agree: [`injected_environment`] supplies a reader without a
/// clock, which is exactly the distinction that keeps clock-reading fixtures
/// out of the rows that pass no [`StdlibConfig`].
#[derive(Clone, Copy, Debug, Default)]
pub struct Capabilities {
    /// The seams the loader accepts from its caller.
    pub seams: Seams,
    /// The loader loads from a workspace path rather than from a string.
    pub from_workspace: bool,
    /// The loader registers a refusing stub in place of every helper that
    /// inspects or acts on the host.
    pub refusing_helpers: bool,
}

/// The injected boundaries a loader accepts from its caller.
#[derive(Clone, Copy, Debug, Default)]
pub struct Seams {
    /// The loader accepts an injected environment reader.
    pub environment: bool,
    /// The loader accepts a [`StdlibConfig`], and so a fixed clock.
    pub clock: bool,
}

impl Capabilities {
    /// Report whether the loader can answer a manifest that reads the
    /// environment.
    ///
    /// It answers one either way: by supplying a reader, or by registering a
    /// refusing stub in place of the helper that would have consulted the
    /// process environment. Refusal counts because it is still a repeatable
    /// outcome — the corpus holds the refusal rows precisely to pin the
    /// diagnostic a caller sees when the boundary is withheld.
    pub const fn resolves_environment_hazard(self) -> bool {
        self.seams.environment || self.refusing_helpers
    }

    /// Report whether the loader can answer a manifest that consults the clock.
    ///
    /// The reasoning mirrors [`resolves_environment_hazard`](Self::resolves_environment_hazard):
    /// a supplied clock and a refusing stub both replace a host read with a
    /// fixed outcome.
    pub const fn resolves_clock_hazard(self) -> bool {
        self.seams.clock || self.refusing_helpers
    }
}

/// Return a loader that reads the process environment and the host clock.
///
/// The string entry points that take no seam, and the path adapters that build
/// their own configuration from the workspace they open, all behave this way.
pub const fn process_environment() -> Capabilities {
    Capabilities {
        seams: Seams {
            environment: false,
            clock: false,
        },
        from_workspace: false,
        refusing_helpers: false,
    }
}

/// Return a loader that opens a workspace but takes no other seam.
pub const fn path_entry_point() -> Capabilities {
    Capabilities {
        from_workspace: true,
        ..process_environment()
    }
}

/// Return a loader that takes an injected environment reader only.
///
/// Rows that pass a reader but no [`StdlibConfig`] — including the policy-taking
/// and budget-taking string adapters — all have these capabilities.
pub const fn injected_environment() -> Capabilities {
    Capabilities {
        seams: Seams {
            environment: true,
            clock: false,
        },
        ..process_environment()
    }
}

/// Return a loader that takes both injected seams.
///
/// Only the rows that pass a [`StdlibConfig`] qualify, which is what makes this
/// distinct from [`injected_environment`]: a clock-carrying fixture is loadable
/// here and not there.
pub const fn injected_environment_and_clock() -> Capabilities {
    Capabilities {
        seams: Seams {
            environment: true,
            clock: true,
        },
        ..process_environment()
    }
}

/// Return a workspace-opening loader that also takes an injected reader.
pub const fn path_entry_point_with_environment() -> Capabilities {
    Capabilities {
        seams: Seams {
            environment: true,
            clock: false,
        },
        ..path_entry_point()
    }
}

/// Return a reader that answers one manifest variable and refuses the rest.
pub fn one_var_reader() -> EnvReader {
    Arc::new(|name: &str| match name {
        "NETSUKE_TEST_ENV" => Ok("characterization".to_owned()),
        _ => Err(EnvReadError::NotPresent),
    })
}

/// Bundle a reader with the permissive access policy the adapters default to.
pub fn environment(reader: &EnvReader) -> ManifestEnvironment<'_> {
    ManifestEnvironment::new(reader, EnvAccessPolicy::default())
}

/// Build the stdlib configuration every clock-carrying row binds.
///
/// The workspace root is the test process's current directory, which no fixture
/// here reads: the configuration is carried for its clock alone. The path
/// override is emptied for the same reason — `which()` is a host lookup, and a
/// row that never calls it should not be able to vary with the host's `PATH`.
///
/// # Errors
///
/// Returns an error when the ambient workspace cannot be opened.
pub fn fixed_clock_config() -> anyhow::Result<StdlibConfig> {
    Ok(StdlibConfig::from_current_dir()?
        .with_clock(fixed_clock(FIXED_INSTANT))
        .with_path_override(std::ffi::OsString::new()))
}

/// Return ceilings that exhaust before any manifest is parsed.
///
/// Exhaustion is charged against the source bytes first, so a limit below the
/// manifest's own text fails ahead of YAML parsing, Jinja evaluation, and
/// rendering. The limit is a constant, which keeps the diagnostic repeatable.
pub fn exhausted_budget() -> ManifestBudgetLimits {
    ManifestBudgetLimits {
        source_bytes: 8,
        ..ManifestBudgetLimits::default()
    }
}
