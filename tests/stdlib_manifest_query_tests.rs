//! The manifest-query stdlib surface stays coherent across helpers.
//!
//! `netsuke help targets` registers a restricted stdlib
//! (`stdlib::register_manifest_query`) instead of the full one. Nothing
//! structurally compares the two surfaces, so a helper whose *arity* is
//! changed in one place and not the other would still render — with the wrong
//! diagnostic. These cases pin the contract for the helper that moved: a
//! disabled helper must report that it is disabled, not that its arguments were
//! wrong, and the helpers the query surface permits must still render.
//!
//! The assertions drive a real `netsuke` process rather than the registration
//! function directly: `register_manifest_query` is crate-private, and the
//! boundary a manifest author actually meets is the command. A target
//! `description` is rendered by both loaders, so it is the field that can
//! distinguish "disabled here" from "wrong arguments everywhere".

use anyhow::{Context, Result, ensure};
use serde_json::Value;
use test_support::fs as test_fs;

/// The marker `src/stdlib/register.rs` attaches to every disabled helper.
const DISABLED_MARKER: &str = "is disabled while rendering";

/// Captured output of one query against `template` as a target description.
struct QueryRun {
    /// Whether the command succeeded.
    success: bool,
    /// Raw stdout, which carries the catalogue on success.
    stdout: String,
    /// Raw stderr, which carries the JSON diagnostic on failure.
    stderr: String,
}

/// Write a one-target manifest whose description is `template`.
fn write_description_manifest(template: &str) -> Result<(tempfile::TempDir, std::path::PathBuf)> {
    let temp = tempfile::tempdir().context("create manifest-query workspace")?;
    let manifest_path = temp.path().join("Netsukefile");
    test_fs::write(
        &manifest_path,
        format!(
            concat!(
                "netsuke_version: \"1.0.0\"\n",
                "targets:\n",
                "  - name: discovery\n",
                "    description: \"{template}\"\n",
                "    command: echo discovery\n",
            ),
            template = template
        ),
    )
    .context("write manifest-query manifest")?;
    Ok((temp, manifest_path))
}

/// Run `netsuke --json help targets` against `template`.
fn run_query(template: &str) -> Result<QueryRun> {
    let (temp, manifest_path) = write_description_manifest(template)?;
    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .current_dir(temp.path())
        .arg("--json")
        .arg("--file")
        .arg(&manifest_path)
        .arg("help")
        .arg("targets")
        .output()
        .context("run netsuke --json help targets")?;
    Ok(QueryRun {
        success: output.status.success(),
        stdout: String::from_utf8(output.stdout).context("stdout should be valid UTF-8")?,
        stderr: String::from_utf8(output.stderr).context("stderr should be valid UTF-8")?,
    })
}

/// Run `template` as a description and return the diagnostic's cause chain.
///
/// A failure is required: the argument-count regression this test exists to
/// exclude is raised while the manifest loads, so a successful run could never
/// be the interesting case.
fn query_manifest_causes(template: &str) -> Result<Vec<String>> {
    let run = run_query(template)?;
    ensure!(
        !run.success,
        "a disabled helper must fail the query: {}",
        run.stdout
    );
    let document: Value = serde_json::from_str(&run.stderr)
        .with_context(|| format!("stderr should be one JSON document: {}", run.stderr))?;
    let causes = document
        .pointer("/diagnostics/0/causes")
        .and_then(Value::as_array)
        .context("diagnostic should carry its cause chain")?
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    Ok(causes)
}

/// Assert that `template` renders under the *full* stdlib.
///
/// This is the negative control for [`query_manifest_causes`]: the same
/// template goes through a build load, so a template that fails there too
/// would make the query assertions vacuous.
fn assert_full_stdlib_renders(template: &str) -> Result<()> {
    let (temp, manifest_path) = write_description_manifest(template)?;
    // `--file` is a global option, so it precedes the subcommand.
    let run = test_support::netsuke::run_netsuke_in(
        temp.path(),
        &[
            "--file",
            manifest_path
                .to_str()
                .context("manifest path should be UTF-8")?,
            "generate",
            "--output",
            "out.ninja",
        ],
    )?;
    ensure!(
        run.success,
        "the full stdlib should render {template:?}: {}",
        run.stderr
    );
    Ok(())
}

/// A disabled helper reports that it is disabled, not an argument error.
///
/// The `env` stub's arity changed when `default=` was added. Had the stub kept
/// its single-argument shape, `env('X', default='y')` would have failed with a
/// detail-free `too many arguments`, naming neither the helper nor the remedy.
/// This asserts the specific marker so the arity cannot silently drift again.
#[test]
fn query_surface_reports_env_as_disabled_with_its_keyword_argument() -> Result<()> {
    let causes = query_manifest_causes("{{ env('NETSUKE_QUERY_PROBE', default='y') }}")?;
    let reported = causes.join("\n");
    ensure!(
        reported.contains(DISABLED_MARKER),
        "env should report itself disabled: {causes:?}"
    );
    ensure!(
        reported.contains("env is disabled"),
        "the disabled diagnostic should name the helper: {causes:?}"
    );
    ensure!(
        !reported.contains("too many arguments") && !reported.contains("missing argument"),
        "the env stub must accept the keyword, not reject it by arity: {causes:?}"
    );
    Ok(())
}

/// The `env` stub keeps its disabled diagnostic without any argument at all.
#[test]
fn query_surface_reports_env_as_disabled_without_arguments() -> Result<()> {
    let causes = query_manifest_causes("{{ env('NETSUKE_QUERY_PROBE') }}")?;
    ensure!(
        causes.join("\n").contains("env is disabled"),
        "a bare env call should also report itself disabled: {causes:?}"
    );
    Ok(())
}

/// The negative control: the same template works under the full stdlib.
///
/// Without this, the assertions above would pass for a manifest that fails in
/// both loaders for an unrelated reason.
#[test]
fn the_full_stdlib_renders_the_same_env_call() -> Result<()> {
    assert_full_stdlib_renders("{{ env('NETSUKE_QUERY_PROBE', default='y') }}")
}

/// Helpers the query surface deliberately permits must render there.
///
/// The shell helpers arrive with EP-M4; naming the currently permitted set
/// here keeps the obligation honest as the surface grows, since a helper added
/// to only one registration path is exactly the drift this contract exists to
/// catch.
#[test]
fn query_surface_renders_its_permitted_helpers() -> Result<()> {
    for (template, expected) in [
        ("{{ ['b', 'a'] | sort | join(',') }}", "a,b"),
        ("{{ 'a b' | upper }}", "A B"),
        ("{{ ['a', 'a'] | unique | join(',') }}", "a"),
        // `compact` is registered by the shared `collections::register_filters`,
        // so it reaches this surface without a second registration; this case
        // is what holds that claim to account.
        ("{{ ['a', '', none, 0] | compact | join(',') }}", "a,0"),
    ] {
        let run = run_query(template)?;
        ensure!(
            run.success,
            "the query surface should render {template:?}: {}",
            run.stderr
        );
        ensure!(
            run.stdout.contains(expected),
            "the query catalogue should contain {expected:?}: {}",
            run.stdout
        );
        assert_full_stdlib_renders(template)?;
    }
    Ok(())
}

/// Fields of one probe expression, separated by `PROBE_SEPARATOR`.
///
/// Two trios, each a join or a quote rendered three times: once with an
/// explicit `sh`, once with an explicit `powershell`, and once with the dialect
/// omitted. The explicit pair pins what each encoder produces; the omitted
/// field must equal whichever of the pair the host's default dialect selects:
///
/// | index | expression                                        |
/// |-------|---------------------------------------------------|
/// | 0     | `shell_join` of the list, `dialect='sh'`          |
/// | 1     | `shell_join` of the list, `dialect='powershell'`  |
/// | 2     | `shell_join` of the list, dialect omitted         |
/// | 3     | `shell_quote` of the word, `dialect='sh'`         |
/// | 4     | `shell_quote` of the word, `dialect='powershell'` |
/// | 5     | `shell_quote` of the word, dialect omitted        |
///
/// The trios exist because a probe that names its dialect *cannot* detect a
/// wrong default: `dialect=` overrides the registration's dialect outright, so
/// seeding `register_query_helpers` with the PowerShell dialect leaves fields
/// 0, 1, 3, and 4 untouched. Fields 2 and 5 are the only ones that read the
/// default the divergence is about, and they are the reason each omitted probe
/// carries explicit twins.
///
/// NOTE: a `|` cannot separate the fields because a POSIX field's output
/// contains one; a backslash cannot because a PowerShell field's output does.
/// `@` appears in neither dialect's output.
const PROBES: &str = concat!(
    "{{ ['target-cpu=native', 'a b'] | shell_join(dialect='sh') }}@",
    "{{ ['target-cpu=native', 'a b'] | shell_join(dialect='powershell') }}@",
    "{{ ['target-cpu=native', 'a b'] | shell_join }}@",
    "{{ 'a b' | shell_quote(dialect='sh') }}@",
    "{{ 'a b' | shell_quote(dialect='powershell') }}@",
    "{{ 'a b' | shell_quote }}",
);

/// The field separator [`PROBES`] uses.
const PROBE_SEPARATOR: &str = "@";

/// The three explicit-dialect fields, as `(label, rendered)`.
///
/// Measured on this host by probe, not derived. `shell_join` quotes each
/// element separately, and the two dialect families differ in kind: `sh`
/// *fragments* around the metacharacter, leaving the longest safe prefix bare,
/// while PowerShell encloses each whole element in single quotes.
const fn expected_explicit_fields() -> [(usize, &'static str, &'static str); 4] {
    [
        (0, "sh shell_join", "target-cpu'=native' a' b'"),
        (1, "powershell shell_join", "'target-cpu=native' 'a b'"),
        (3, "sh shell_quote", "a' b'"),
        (4, "powershell shell_quote", "'a b'"),
    ]
}

/// The `(explicit sh field, explicit powershell field, omitted field)` trios.
///
/// The host's `RecipeShell::host_default` picks which of the first two the
/// third must equal: `PowerShell` on Windows, `Posix` — which shares the `Sh`
/// dialect with `Bash` — elsewhere.
const TRIOS: [(usize, usize, usize); 2] = [(0, 1, 2), (3, 4, 5)];

/// Run `PROBES` as a description and split its rendering into fields.
fn probe_fields() -> Result<Vec<String>> {
    let run = run_query(PROBES)?;
    ensure!(
        run.success,
        "the query surface should render the shell probes: {}",
        run.stderr
    );
    let document: Value = serde_json::from_str(&run.stdout)
        .with_context(|| format!("stdout should be one JSON document: {}", run.stdout))?;
    let description = document
        .pointer("/result/targets/0/description")
        .and_then(Value::as_str)
        .context("the catalogue should carry the target description")?;
    let fields = description
        .split(PROBE_SEPARATOR)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    ensure!(
        fields.len() == 6,
        "the description should split into six probes, got {}: {description:?}",
        fields.len()
    );
    Ok(fields)
}

/// The query surface quotes exactly as the build surface does.
///
/// This is the assertion the plan calls for, and it is *not* the one the plan
/// imagined. `register_query_helpers` documents the divergence as deliberate —
/// the query surface quotes for [`RecipeShell::host_default`] rather than for
/// the shell the build resolves — but here the two surfaces cannot actually
/// disagree about *any* dialect, for a reason the plan's rationale omits:
///
/// * `resolve_recipe_shell_with` returns [`RecipeShell::Posix`] on a non-Windows
///   host *before* it reads `NETSUKE_WINDOWS_SHELL`, so on Unix a malformed
///   value cannot be reached at all and the shell is always the host default.
/// * On Windows there is no early return, but `execute_help` returns from the
///   dispatcher before `resolve_recipe_shell` is ever called, so the query
///   never resolves it either — and `registry`, the crate-private reference
///   `register_query_helpers` uses, *is* `host_default()`.
///
/// So the plan's stated hazard — that hoisting the resolution would "make
/// `netsuke help targets` fail on a Windows host with a malformed
/// `NETSUKE_WINDOWS_SHELL`" — is unreachable either way, and this test pins the
/// agreement that actually holds: given an explicit dialect, a `shell_quote` or
/// `shell_join` in a description renders identically under the full stdlib, on
/// every host.
///
/// `assert_full_stdlib_renders` is the negative control: if the probe failed
/// under the full stdlib too, the comparison below would be satisfied by two
/// identical failures.
#[test]
fn query_surface_agrees_with_the_build_on_explicit_dialects() -> Result<()> {
    let fields = probe_fields()?;
    for (index, label, expected) in expected_explicit_fields() {
        let observed = fields
            .get(index)
            .with_context(|| format!("probe {index} ({label}) should have rendered"))?;
        ensure!(
            observed == expected,
            "the query surface rendered probe {index} ({label}) as {observed:?}, expected {expected:?}"
        );
    }
    assert_full_stdlib_renders(PROBES)?;
    Ok(())
}

/// The index of the trio field that carries the dialect the host resolves to.
///
/// `RecipeShell::host_default` is `PowerShell` on Windows and `Posix` — which
/// shares the `Sh` dialect with `Bash` — everywhere else. Mirroring that `cfg!`
/// here is not a second guess at the host: it is the same predicate restated
/// where a reader of this contract can check it, and it is what makes the
/// assertion below falsifiable. A membership test over *both* twins would not
/// be — seeding `register_query_helpers` with the PowerShell dialect satisfies
/// it on a Unix host, because the omitted field then equals the PowerShell
/// twin.
const fn host_default_field(trio: (usize, usize, usize)) -> usize {
    let (sh_index, power_shell_index, default_index) = trio;
    if cfg!(windows) {
        let _ = (sh_index, default_index);
        power_shell_index
    } else {
        let _ = (power_shell_index, default_index);
        sh_index
    }
}

/// The dialect-omitting probes resolve to the host's default dialect.
///
/// The assertion a wrong default trips, and the explicit-dialect one cannot:
/// `dialect=` overrides the registration's dialect, so seeding
/// `register_query_helpers` with `RecipeShell::PowerShell` leaves every
/// explicit probe intact. The defect would reach a user as silently mismatched
/// quoting between `netsuke help targets` and the build that consumes the same
/// manifest.
///
/// This file runs on Windows too — `make SHELL=bash test` is a merge gate
/// (`.github/workflows/ci-windows.yml`) — so the expected twin is chosen by
/// [`host_default_field`] rather than hardcoded to `sh`.
#[test]
fn query_surface_renders_the_host_resolved_dialect() -> Result<()> {
    let fields = probe_fields()?;
    for trio in TRIOS {
        let (sh_index, power_shell_index, default_index) = trio;
        let sh = fields
            .get(sh_index)
            .with_context(|| format!("probe {sh_index} (explicit sh) should have rendered"))?;
        let power_shell = fields.get(power_shell_index).with_context(|| {
            format!("probe {power_shell_index} (explicit powershell) should have rendered")
        })?;
        let default = fields.get(default_index).with_context(|| {
            format!("probe {default_index} (default dialect) should have rendered")
        })?;

        // The twins must differ, or the comparison below would be satisfied by
        // a single dialect family however the default resolved.
        ensure!(
            sh != power_shell,
            "probes {sh_index} and {power_shell_index} should render differently, \
             got {sh:?} for both"
        );

        let expected = fields
            .get(host_default_field(trio))
            .context("the host-default field index should be in range")?;
        ensure!(
            default == expected,
            "the dialect-omitting probe {default_index} rendered {default:?}, but this host's \
             default dialect produces {expected:?}; sh renders {sh:?} and powershell renders \
             {power_shell:?}"
        );
    }
    Ok(())
}
