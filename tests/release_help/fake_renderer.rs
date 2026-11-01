//! Embedded fake `cargo-orthohelp` used by the release-help script tests.
//!
//! The script under test is exercised against this renderer so the tests need
//! no network access and no real `cargo-orthohelp` installation. Keeping the
//! script text in its own module leaves the fixture module below the
//! repository's module-size limit.

const FAKE_CARGO_ORTHOHELP_PROLOGUE: &str = r#"#!/usr/bin/env bash
set -euo pipefail

printf '%s\n' "$*" >>"${ORTHOHELP_FAKE_LOG}"

if [[ "${ORTHOHELP_FAKE_FAIL:-0}" == "1" ]]; then
  echo "fake cargo-orthohelp failure" >&2
  exit 42
fi

format=""
out_dir=""
module_name="Netsuke"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --format)
      format="$2"
      shift 2
      ;;
    --out-dir)
      out_dir="$2"
      shift 2
      ;;
    --ps-module-name)
      module_name="$2"
      shift 2
      ;;
    *)
      shift
      ;;
  esac
done

if [[ "${ORTHOHELP_FAKE_SKIP_OUTPUT:-}" == "$format" ]]; then
  exit 0
fi

"#;

const FAKE_CARGO_ORTHOHELP_METADATA: &str = r#"mapfile -t public_cli_surface < <(
  python3 - "${ORTHOHELP_FAKE_METADATA:?release-help metadata is required}" <<'PY'
import json
import os
import sys

with open(sys.argv[1], encoding="utf-8") as metadata_file:
    metadata = json.load(metadata_file)

# Mirrors a renderer that silently drops an option, so the script's own
# rendered-surface verification can be proven live.
omit_long = os.environ.get("ORTHOHELP_FAKE_OMIT_FLAG", "")

def render_flag(field):
    """Render one documented CLI flag as the generated help spells it."""
    cli = field["cli"]
    long = cli["long"]
    if not long:
        raise ValueError(f"documented {field['name']} metadata must have a long flag")
    rendered = f"-{cli['short']}/--{long}" if cli["short"] else f"--{long}"
    if cli["takes_value"]:
        rendered += f" {cli['value_name'] or 'VALUE'}"
        possible_values = cli["possible_values"]
        if possible_values:
            rendered += " " + "|".join(possible_values)
    return rendered


documented_fields = [
    field
    for field in metadata["fields"]
    if field["cli"] and field["cli"]["long"] != omit_long
]
parser_only_fields = [
    field
    for field in documented_fields
    if field["env"] is None and field["file"] is None
]
if not parser_only_fields:
    raise ValueError("release-help metadata must include parser-only selectors")

value_placeholders = []
for field in documented_fields:
    cli = field.get("cli")
    if not cli or not cli.get("value_name"):
        continue
    value_placeholders.append(f"{cli['long']}={cli['value_name']}")
if not value_placeholders:
    raise ValueError("release-help metadata must declare value placeholders")

# Every documented flag appears in the summary line, switches included. Only
# the value-taking ones also get a detail block below, mirroring how the real
# renderer lists an option and then its placeholder.
print(" ".join(render_flag(field) for field in documented_fields))
print(" ".join(command["app_name"] for command in metadata["subcommands"]))
print(" ".join(value_placeholders))
PY
)
documented_flags="${public_cli_surface[0]}"
subcommands="${public_cli_surface[1]}"
value_placeholders="${public_cli_surface[2]}"

"#;

const FAKE_CARGO_ORTHOHELP_OUTPUT: &str = r#"case "$format" in

  man)
    mkdir -p "$out_dir/man/man1"
    cat >"$out_dir/man/man1/netsuke.1" <<'MAN'
.TH NETSUKE 1 "1970-01-01"
.SH NAME
netsuke \- dependency-aware build orchestration
.SH SYNOPSIS
.B netsuke
[OPTIONS]
MAN
    cat >>"$out_dir/man/man1/netsuke.1" <<MAN
.SH OPTIONS
.B $documented_flags
MAN
    for pair in $value_placeholders; do
      long="${pair%%=*}"
      value="${pair#*=}"
      printf '.TP\n\\fB--%s\\fR \\fI%s\\fP\n' "$long" "$value" >>"$out_dir/man/man1/netsuke.1"
    done
    if [[ "${ORTHOHELP_FAKE_FALLBACK_FORMAT:-}" == "man" && -n "${ORTHOHELP_FAKE_FALLBACK:-}" ]]; then
      printf '.TP\n\\fB--fake-fallback\\fR \\fI%s\\fP\n' "$ORTHOHELP_FAKE_FALLBACK" >>"$out_dir/man/man1/netsuke.1"
    fi
    cat >>"$out_dir/man/man1/netsuke.1" <<MAN
.SH COMMANDS
$subcommands
MAN
    ;;
  ps)
    help_dir="$out_dir/powershell/$module_name/en-US"
    mkdir -p "$help_dir"
    printf 'function Invoke-%s { param() }\n' "$module_name" >"$out_dir/powershell/$module_name/$module_name.psm1"
    printf '@{\n  RootModule = "%s.psm1"\n  ModuleVersion = "0.1.0"\n}\n' "$module_name" >"$out_dir/powershell/$module_name/$module_name.psd1"
    {
      printf '<helpItems schema="maml">\n'
      # Both prefixes are declared on the command element so the sibling
      # `maml:para` and `maml:name` elements inherit a binding. Declaring
      # `xmlns:maml` on the paragraph instead would leave every `maml:name`
      # unbound and the document unparseable, which the test now catches.
      printf '  <command:command xmlns:command="http://schemas.microsoft.com/maml/dev/command/2004/10" xmlns:maml="http://schemas.microsoft.com/maml/2004/10">\n'
      printf '    <command:details>\n      <command:name>%s</command:name>\n    </command:details>\n' "$module_name"
      printf '    <command:description>\n      <maml:para>%s %s</maml:para>\n    </command:description>\n' "$documented_flags" "$subcommands"
      for pair in $value_placeholders; do
        long="${pair%%=*}"
        value="${pair#*=}"
        printf '    <command:parameter required="true"><maml:name>--%s</maml:name><command:parameterValue required="true">%s</command:parameterValue></command:parameter>\n' "$long" "$value"
      done
      if [[ "${ORTHOHELP_FAKE_FALLBACK_FORMAT:-}" == "ps" && -n "${ORTHOHELP_FAKE_FALLBACK:-}" ]]; then
        printf '    <command:parameter required="true"><maml:name>--fake-fallback</maml:name><command:parameterValue required="true">%s</command:parameterValue></command:parameter>\n' "$ORTHOHELP_FAKE_FALLBACK"
      fi
      printf '  </command:command>\n</helpItems>\n'
    } >"$help_dir/$module_name-help.xml"
    printf 'TOPIC\n    about_%s\n' "$module_name" >"$help_dir/about_$module_name.help.txt"
    ;;
esac
"#;

/// Build the fake cargo-orthohelp executable script from its components.
pub fn fake_cargo_orthohelp_script() -> String {
    [
        FAKE_CARGO_ORTHOHELP_PROLOGUE,
        FAKE_CARGO_ORTHOHELP_METADATA,
        FAKE_CARGO_ORTHOHELP_OUTPUT,
    ]
    .concat()
}
