//! Contracts for the deny-by-default template surface used by `netsuke test`.
//!
//! Three claims are asserted here, and they are kept together because each is
//! only meaningful beside the other two. The inventory is complete: every
//! helper registered anywhere under `src/stdlib` appears in `SURFACE`, and no
//! name the inventory claims to refuse is reachable as an unknown helper. Every
//! row has a probe that loads a real manifest through the loader, so the
//! surface is observed where a manifest author meets it rather than through a
//! reconstruction of the registration. And every refusal is typed: the error
//! source downcasts to [`TestModeRefusal`], which is what lets a caller tell a
//! withheld capability from a typo without matching diagnostic text.
//!
//! The probe table is keyed by `(name, kind)` rather than name alone because
//! `which` is both a filter and a function, and the two are separate
//! registrations that can regress independently.
//!
//! These rows live beside `tests.rs` rather than inside it: that module pins
//! the query-refusal text and already fills most of the 400-line file cap, so
//! extending it would have taken it over the limit. The split is mechanical —
//! the query rows are untouched — and the two modules share only `SURFACE`.

use std::{error::Error as _, sync::Arc};

use anyhow::{Context as _, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::{Environment, Error as MiniJinjaError, ErrorKind};
use rstest::rstest;
use test_support::{
    EnLocalizer, en_localizer, fluent::normalize_fluent_isolates, localizer::locale_localizer,
};

use super::{
    HelperEntry, HelperKind, RestrictedSurface, SURFACE, TestDisposition, TestModeRefusal,
    register_for_test,
};
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

/// Check one inventory row against the probe's outcome.
fn check_row(entry: &HelperEntry, expression: &str) -> std::result::Result<(), String> {
    let outcome = load_probe(expression);
    match entry.test {
        TestDisposition::Available => outcome
            .map(|_| ())
            .map_err(|error| format!("expected `{expression}` to load, got {error:#}")),
        TestDisposition::Refused | TestDisposition::ClockInjected => match outcome {
            Ok(_) => Err(format!("expected `{expression}` to refuse, but it loaded")),
            Err(error) => check_refusal(entry, &error),
        },
    }
}

/// Report whether `kind` is `MiniJinja`'s unknown-helper error.
///
/// An inventory row whose helper was never registered resolves as unknown
/// rather than refused, which is a different fault from a misnamed refusal and
/// is reported with its own message.
fn is_unknown_helper(kind: ErrorKind) -> bool {
    matches!(
        kind,
        ErrorKind::UnknownFunction | ErrorKind::UnknownFilter | ErrorKind::UnknownTest
    )
}

/// Check a refused probe names its helper and is not an unknown-helper error.
fn check_refusal(entry: &HelperEntry, error: &anyhow::Error) -> std::result::Result<(), String> {
    if let Some(kind) = jinja_error(error)
        .map(MiniJinjaError::kind)
        .filter(|kind| is_unknown_helper(*kind))
    {
        return Err(format!(
            "`{}` resolved as an unknown helper: {kind:?}",
            entry.name
        ));
    }
    match refusal_operation(error) {
        Some(operation) if operation == entry.name => Ok(()),
        Some(operation) => Err(format!("`{}` refused as `{operation}`", entry.name)),
        None => Err(format!(
            "`{}` failed without a typed refusal: {error:#}",
            entry.name
        )),
    }
}

/// Assert every inventory row can be exercised and answers as recorded.
#[test]
fn every_inventory_row_has_a_reachable_probe() {
    let mut failures = Vec::new();
    for entry in SURFACE {
        match probe_expression(entry) {
            Some(expression) => {
                if let Err(failure) = check_row(entry, expression) {
                    failures.push(format!("{} ({:?}): {failure}", entry.name, entry.kind));
                }
            }
            None => failures.push(format!(
                "{} ({:?}): no probe expression is defined",
                entry.name, entry.kind
            )),
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} inventory rows failed:\n{}",
        failures.len(),
        SURFACE.len(),
        failures.join("\n")
    );
}

/// Return every literal `add_function`/`add_filter`/`add_test` registration.
///
/// The scanner walks the source text rather than the compiled environment
/// because `MiniJinja` exposes no way to enumerate filters or tests, so the
/// registrations themselves are the only complete witness. Whitespace between
/// the call, the parenthesis, and the name literal is tolerated so a reformat
/// cannot silently hide a name.
fn scanned_registrations() -> Result<Vec<(Utf8PathBuf, String)>> {
    const MARKERS: [&str; 3] = ["add_function", "add_filter", "add_test"];
    let root = Dir::open_ambient_dir(env!("CARGO_MANIFEST_DIR"), ambient_authority())
        .context("open the crate root")?;
    let mut sources = Vec::new();
    collect_sources(&root, Utf8Path::new("src/stdlib"), &mut sources)?;
    let mut found = Vec::new();
    for path in sources {
        let text = root
            .read_to_string(path.as_str())
            .with_context(|| format!("read {path}"))?;
        for name in literal_names(&text, &MARKERS) {
            found.push((path.clone(), name));
        }
    }
    Ok(found)
}

/// Append every Rust source beneath `directory` to `sources`.
fn collect_sources(root: &Dir, directory: &Utf8Path, sources: &mut Vec<Utf8PathBuf>) -> Result<()> {
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
            collect_sources(root, &path, sources)?;
        } else if path.extension() == Some("rs") {
            sources.push(path);
        }
    }
    Ok(())
}

/// Return every `marker("name"` occurrence in `text`.
#[expect(
    clippy::string_slice,
    reason = "every offset comes from `str::find`, so each one is a character boundary"
)]
fn literal_names(text: &str, markers: &[&str]) -> Vec<String> {
    let mut names = Vec::new();
    for marker in markers {
        let mut from = 0;
        while let Some(offset) = text[from..].find(marker) {
            let after = from + offset + marker.len();
            if let Some(name) = literal_after(&text[after..]) {
                names.push(name);
            }
            from = after;
        }
    }
    names
}

/// Return the string literal following `(` and whitespace, if one is there.
#[expect(
    clippy::string_slice,
    reason = "the end offset comes from `str::find`, so it is a character boundary"
)]
fn literal_after(rest: &str) -> Option<String> {
    let after_paren = rest.trim_start().strip_prefix('(')?.trim_start();
    let quoted = after_paren.strip_prefix('"')?;
    let end = quoted.find('"')?;
    Some(quoted[..end].to_owned())
}

/// Assert every literal registration appears in `SURFACE`.
#[test]
fn every_scanned_registration_is_inventoried() -> Result<()> {
    let scanned = scanned_registrations()?;
    ensure!(
        !scanned.is_empty(),
        "the source scan found no registrations; the walk is broken"
    );
    let mut unexpected: Vec<String> = scanned
        .iter()
        .filter(|(_, name)| !SURFACE.iter().any(|entry| entry.name == name))
        .map(|(path, name)| format!("`{name}` in {path}"))
        .collect();
    unexpected.sort();
    ensure!(
        unexpected.is_empty(),
        "literal registrations missing from `SURFACE`: {unexpected:?}"
    );
    Ok(())
}

/// Assert every invented name has a registration site or a stated exemption.
#[test]
fn every_inventory_name_is_registered_somewhere() -> Result<()> {
    let scanned = scanned_registrations()?;
    let mut missing: Vec<&str> = SURFACE
        .iter()
        .filter(|entry| {
            let is_loader_owned = matches!(entry.name, "env" | "glob");
            let is_loop_registered = entry.kind == HelperKind::Test;
            let is_scanned = scanned.iter().any(|(_, name)| name == entry.name);
            !(is_loader_owned || is_loop_registered || is_scanned)
        })
        .map(|entry| entry.name)
        .collect();
    missing.sort_unstable();
    ensure!(
        missing.is_empty(),
        "inventory names with no registration site: {missing:?}"
    );
    Ok(())
}

/// Build an environment holding only the test surface and its refusals.
fn refusal_environment() -> Environment<'static> {
    let mut env = Environment::new();
    let config = probe_config();
    let _state = register_for_test(&mut env, &config);
    super::register_glob_refusal(&mut env, RestrictedSurface::Test);
    env
}

/// Assert a rendered expression refuses with the typed source and operation.
fn assert_typed_refusal(env: &Environment<'_>, expression: &str, operation: &str) -> Result<()> {
    let error = env
        .render_str(expression, ())
        .expect_err(&format!("`{expression}` must refuse"));
    ensure!(
        error.kind() == ErrorKind::InvalidOperation,
        "`{expression}` reported {kind:?}, not an invalid operation",
        kind = error.kind()
    );
    let refusal = error
        .source()
        .and_then(|source| source.downcast_ref::<TestModeRefusal>())
        .with_context(|| format!("`{expression}` carried no typed refusal"))?;
    ensure!(
        refusal.operation() == operation,
        "`{expression}` refused as `{}`, not `{operation}`",
        refusal.operation()
    );
    Ok(())
}

/// Assert every refusing row carries a typed source naming the helper.
#[test]
fn refusals_carry_a_typed_source() -> Result<()> {
    let env = refusal_environment();
    let mut failures = Vec::new();
    for entry in SURFACE
        .iter()
        .filter(|entry| RestrictedSurface::Test.refuses(entry))
    {
        let Some(expression) = probe_expression(entry) else {
            failures.push(format!("{}: no probe expression", entry.name));
            continue;
        };
        if let Err(error) = assert_typed_refusal(&env, expression, entry.name) {
            failures.push(format!("{}: {error:#}", entry.name));
        }
    }
    ensure!(
        failures.is_empty(),
        "{} refusing rows lacked a typed source:\n{}",
        failures.len(),
        failures.join("\n")
    );
    Ok(())
}

/// Assert a refusal is reached from both positional and keyword call shapes.
#[test]
fn refusals_accept_any_call_shape() -> Result<()> {
    let env = refusal_environment();
    for expression in [
        "{{ fetch('https://example.invalid') }}",
        "{{ fetch(url='https://example.invalid') }}",
        "{{ 'probe' | hash('sha256') }}",
        "{{ 'probe' | hash() }}",
        "{{ 'probe' is file }}",
    ] {
        assert_typed_refusal(&env, expression, expr_operation(expression))?;
    }
    Ok(())
}

/// Return the helper name an expression in the call-shape table exercises.
fn expr_operation(expression: &str) -> &'static str {
    if expression.contains("fetch") {
        "fetch"
    } else if expression.contains("hash") {
        "hash"
    } else {
        "file"
    }
}

/// Pin the English refusal text for one helper.
#[rstest]
fn english_refusal_text_is_pinned(en_localizer: EnLocalizer) {
    let _localizer = en_localizer;
    let text = normalize_fluent_isolates(&TestModeRefusal::new("hash").to_string());
    insta::assert_snapshot!(
        text,
        @"[netsuke::jinja::test_mode::unavailable] hash is unavailable under `netsuke test`."
    );
}

/// Assert the refusal leads with its code under a non-English catalogue.
#[test]
fn refusal_text_carries_the_code_under_another_locale() {
    let _guards = locale_localizer("ja");
    let text = normalize_fluent_isolates(&TestModeRefusal::new("hash").to_string());
    assert!(
        text.starts_with("[netsuke::jinja::test_mode::unavailable] hash"),
        "Japanese refusal did not lead with the code and helper name: {text}"
    );
}
