//! The shell-dialect probe contract for the manifest-query surface.
//!
//! The probes and their assertions live here rather than in the crate root
//! because together they are the larger half of that contract; the root keeps
//! the obligations a reader meets first — the disabled-helper arity cases and
//! the permitted-helper sweep.
//!
//! Both tests run a real `netsuke` process through the sibling [`run_query`],
//! so this module is where the two registration paths are compared rather than
//! merely described. The trios exist because an explicit `dialect=` overrides
//! the registration's dialect outright: the dialect-omitting probes are the
//! only ones that can detect a wrong default, and the explicit twins are what
//! make their result falsifiable rather than satisfiable by either family.
//!
//! [`run_query`]: super::run_query

use anyhow::{Context, Result, ensure};
use serde_json::Value;

use super::{assert_full_stdlib_renders, run_query};

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
