//! Contracts for the deny-by-default template surface used by `netsuke test`.
//!
//! Three claims are asserted across this module and its children, and they are
//! kept together because each is only meaningful beside the other two. The
//! inventory is complete: every helper registered anywhere under `src/stdlib`
//! appears in `SURFACE`, and no name the inventory claims to refuse is
//! reachable as an unknown helper. Every row has a probe that loads a real
//! manifest through the loader, so the surface is observed where a manifest
//! author meets it rather than through a reconstruction of the registration.
//! And every refusal is typed: the error source downcasts to [`TestModeRefusal`],
//! which is what lets a caller tell a withheld capability from a typo without
//! matching diagnostic text.
//!
//! The probe table is keyed by `(name, kind)` rather than name alone because
//! `which` is both a filter and a function, and the two are separate
//! registrations that can regress independently.
//!
//! These rows live beside `tests.rs` rather than inside it: that module pins
//! the query-refusal text and already fills most of the 400-line file cap, so
//! extending it would have taken it over the limit. The split is mechanical —
//! the query rows are untouched — and the two modules share only `SURFACE`.
//!
//! This module owns the probe fixtures the rows share; `probe_rows.rs` runs
//! them against the inventory, `registration_scan.rs` checks the inventory
//! against the registration sites in the source tree, and `refusal_text.rs`
//! pins the refusal shape and its text. Those three are separate files because
//! together they exceeded the same cap.

mod probe_rows;
mod refusal_text;
mod registration_scan;

use std::{error::Error as _, sync::Arc};

use anyhow::Result;
use minijinja::Error as MiniJinjaError;

use super::{HelperEntry, HelperKind, TestModeRefusal};
use crate::manifest::{
    EnvReadError, EnvReader, ManifestLoadOptions, ManifestName, TestSurface, from_str_with_options,
};
use crate::stdlib::StdlibConfig;

/// Environment name the injected probe reader answers.
const PROBE_ENV_NAME: &str = "NETSUKE_TEST_ENV";
/// Value the injected probe reader returns.
const PROBE_ENV_VALUE: &str = "probe";

/// Build the stdlib configuration the probe surface carries.
fn probe_config() -> StdlibConfig {
    StdlibConfig::from_current_dir().expect("the test workspace must open")
}

/// Return a reader that answers one name and refuses the rest.
fn probe_reader() -> EnvReader {
    Arc::new(|name: &str| {
        if name == PROBE_ENV_NAME {
            Ok(PROBE_ENV_VALUE.to_owned())
        } else {
            Err(EnvReadError::NotPresent)
        }
    })
}

/// Build the test surface every manifest-level probe loads through.
fn probe_surface() -> TestSurface {
    TestSurface::new(probe_config()).with_env_reader(probe_reader())
}

/// Build a one-target manifest whose command interpolates `expression`.
///
/// The command is the rendered field because it is the one place a probe can
/// observe a value without needing a target property the AST does not carry.
fn probe_manifest(expression: &str) -> String {
    format!(
        concat!(
            "netsuke_version: \"1.0.0\"\n",
            "targets:\n",
            "  - name: probe\n",
            "    command: \"echo {}\"\n",
        ),
        expression
    )
}

/// Load the probe manifest under the deny-by-default surface.
fn load_probe(expression: &str) -> Result<crate::ast::NetsukeManifest> {
    from_str_with_options(
        &probe_manifest(expression),
        &ManifestName::new("Netsukefile"),
        ManifestLoadOptions::test(probe_surface()),
    )
}

/// Return the `MiniJinja` error at the root of a load failure, if any.
fn jinja_error(error: &anyhow::Error) -> Option<&MiniJinjaError> {
    error
        .chain()
        .find_map(|cause| cause.downcast_ref::<MiniJinjaError>())
}

/// Return the operation named by a test-mode refusal in the error chain.
fn refusal_operation(error: &anyhow::Error) -> Option<&str> {
    let jinja = jinja_error(error)?;
    jinja
        .source()?
        .downcast_ref::<TestModeRefusal>()
        .map(TestModeRefusal::operation)
}

/// Return the template expression that exercises one inventory row.
fn probe_expression(entry: &HelperEntry) -> Option<&'static str> {
    let expression = match (entry.name, entry.kind) {
        ("env", _) => "{{ env('NETSUKE_TEST_ENV') }}",
        ("glob", _) => "{{ glob('*') }}",
        ("fetch", _) => "{{ fetch('https://example.invalid/probe') }}",
        ("which", HelperKind::Filter) => "{{ 'rustc' | which }}",
        ("which", HelperKind::Function) => "{{ which('rustc') }}",
        ("command_available", _) => "{{ command_available('rustc') }}",
        ("now", _) => "{{ now() }}",
        ("timedelta", _) => "{{ timedelta(seconds=1) }}",
        ("shell", _) => "{{ 'input' | shell('echo', 'hi') }}",
        ("grep", _) => "{{ 'input' | grep('probe') }}",
        ("contents", _) => "{{ 'probe.txt' | contents }}",
        ("realpath", _) => "{{ 'probe' | realpath }}",
        ("expanduser", _) => "{{ '~' | expanduser }}",
        ("size", _) => "{{ 'probe' | size }}",
        ("linecount", _) => "{{ 'probe' | linecount }}",
        ("hash", _) => "{{ 'probe' | hash('sha256') }}",
        ("digest", _) => "{{ 'probe' | digest(8) }}",
        ("basename", _) => "{{ 'a/b.txt' | basename }}",
        ("dirname", _) => "{{ 'a/b.txt' | dirname }}",
        ("with_suffix", _) => "{{ 'a.txt' | with_suffix('.md') }}",
        ("relative_to", _) => "{{ 'reports/daily.csv' | relative_to('reports') }}",
        ("shell_quote", _) => "{{ 'probe' | shell_quote }}",
        ("shell_join", _) => "{{ ['a', 'b'] | shell_join }}",
        ("uniq", _) => "{{ ['a', 'a'] | uniq | join(',') }}",
        ("flatten", _) => "{{ [[1], [2]] | flatten | join(',') }}",
        ("compact", _) => "{{ ['a', '', none] | compact | join(',') }}",
        ("group_by", _) => "{{ ([{'kind': 'tool'}] | group_by('kind')).tool | length }}",
        ("dir", _) => "{{ 'probe' is dir }}",
        ("file", _) => "{{ 'probe' is file }}",
        ("symlink", _) => "{{ 'probe' is symlink }}",
        ("pipe", _) => "{{ 'probe' is pipe }}",
        ("block_device", _) => "{{ 'probe' is block_device }}",
        ("char_device", _) => "{{ 'probe' is char_device }}",
        ("device", _) => "{{ 'probe' is device }}",
        _ => return None,
    };
    Some(expression)
}
