#!/usr/bin/env bash
set -euo pipefail

fallback_date="1970-01-01"
module_name="Netsuke"

usage() {
  echo "usage: scripts/generate-release-help.sh <target> <bin-name> <out-dir>" >&2
}

warn_source_date_epoch() {
  local raw="$1"
  local reason="$2"
  echo "::warning title=Invalid SOURCE_DATE_EPOCH::${raw} ${reason}; falling back to ${fallback_date}" >&2
}

manual_date() {
  local raw="${SOURCE_DATE_EPOCH:-}"
  if [[ -z "$raw" ]]; then
    echo "$fallback_date"
    return
  fi

  if [[ ! "$raw" =~ ^-?[0-9]+$ ]]; then
    warn_source_date_epoch "$raw" "is not integer seconds since the Unix epoch"
    echo "$fallback_date"
    return
  fi

  local python_bin=""
  if command -v python3 >/dev/null 2>&1; then
    python_bin="python3"
  elif command -v python >/dev/null 2>&1; then
    python_bin="python"
  fi

  if [[ -z "$python_bin" ]]; then
    warn_source_date_epoch "$raw" "cannot be converted because Python is unavailable"
    echo "$fallback_date"
    return
  fi

  local formatted
  if ! formatted="$("$python_bin" - "$raw" <<'PY'
import datetime
import sys

try:
    timestamp = int(sys.argv[1])
    instant = datetime.datetime.fromtimestamp(timestamp, tz=datetime.timezone.utc)
except (OverflowError, OSError, ValueError):
    sys.exit(1)

print(instant.date().isoformat())
PY
)"; then
    warn_source_date_epoch "$raw" "is not a valid Unix timestamp"
    echo "$fallback_date"
    return
  fi

  echo "$formatted"
}

require_file() {
  local path="$1"
  local message="$2"
  if [[ ! -f "$path" ]]; then
    echo "::error title=Release help missing::${message}: ${path}" >&2
    exit 1
  fi
}

target_is_windows() {
  local target="$1"
  [[ "$target" == *windows* ]]
}

if [[ $# -ne 3 ]]; then
  usage
  exit 2
fi

target="$1"
bin_name="$2"
out_dir="$3"
man_date="$(manual_date)"

cargo orthohelp \
  --format man \
  --out-dir "$out_dir" \
  --locale en-US \
  --man-section 1 \
  --man-date "$man_date"

require_file "$out_dir/man/man1/${bin_name}.1" "manual page was not generated"

if target_is_windows "$target"; then
  cargo orthohelp \
    --format ps \
    --out-dir "$out_dir" \
    --locale en-US \
    --ps-module-name "$module_name" \
    --ensure-en-us true

  require_file "$out_dir/powershell/$module_name/$module_name.psm1" \
    "PowerShell module script was not generated"
  require_file "$out_dir/powershell/$module_name/$module_name.psd1" \
    "PowerShell module manifest was not generated"
  require_file "$out_dir/powershell/$module_name/en-US/$module_name-help.xml" \
    "PowerShell MAML help was not generated"
  require_file "$out_dir/powershell/$module_name/en-US/about_$module_name.help.txt" \
    "PowerShell about help was not generated"
fi
