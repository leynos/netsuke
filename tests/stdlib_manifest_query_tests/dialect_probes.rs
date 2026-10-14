//! The shell-dialect probe contract for the manifest-query surface.
//!
//! The probes and their assertions live here rather than in the crate root
//! because together they are the larger half of that contract; the root keeps
//! the obligations a reader meets first — the disabled-helper arity cases and
//! the permitted-helper sweep.
//!
//! Both tests run a real graph-bearing `netsuke` query through the sibling
//! [`run_query`]. The explicit probes pin each dialect, while the omitted
//! probes pin the query surface's stable `sh` default on every host.
//!
//! [`run_query`]: super::run_query

use anyhow::{Context, Result, ensure};
use serde_json::Value;

use super::{assert_full_stdlib_renders, run_query};

/// Fields of one probe expression, separated by `PROBE_SEPARATOR`.
///
/// Two trios, each a join or a quote rendered three times: once with an
/// explicit `sh`, once with an explicit `powershell`, and once with the dialect
/// omitted. The omitted fields must match the explicit `sh` fields regardless
/// of the host:
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
/// Explicit twins make the fixed default assertion falsifiable: each pair has
/// different output, so an omitted field cannot accidentally match both.
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
/// Stable output for these inputs. `shell_join` quotes each element separately,
/// and the dialect families differ: `sh` fragments around the metacharacter,
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
    let catalogue_description = document
        .pointer("/result/targets/0/description")
        .and_then(Value::as_str)
        .context("the catalogue should carry the target description")?;
    let graph_description = document
        .pointer("/result/graph/nodes/0/description")
        .and_then(Value::as_str)
        .context("the graph should carry the declaration description")?;
    ensure!(
        graph_description == catalogue_description,
        "catalogue and graph metadata should share one rendered description"
    );
    let fields = graph_description
        .split(PROBE_SEPARATOR)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    ensure!(
        fields.len() == 6,
        "the description should split into six probes, got {}: {graph_description:?}",
        fields.len()
    );
    Ok(fields)
}

/// An explicit dialect overrides the query's fixed default.
///
/// The full stdlib is a negative control: the same templates must remain valid
/// during build rendering even though that surface selects its recipe shell.
#[test]
fn query_surface_honours_explicit_dialects() -> Result<()> {
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

/// The omitted dialect always uses `sh`, including when the host is Windows.
#[test]
fn query_surface_uses_a_stable_sh_default() -> Result<()> {
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

        ensure!(
            sh != power_shell,
            "probes {sh_index} and {power_shell_index} should render differently, \
             got {sh:?} for both"
        );

        ensure!(
            default == sh,
            "the dialect-omitting probe {default_index} rendered {default:?}, but query metadata \
             must use the fixed sh dialect; powershell renders {power_shell:?}"
        );
    }
    Ok(())
}
