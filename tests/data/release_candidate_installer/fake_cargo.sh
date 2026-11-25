#!/usr/bin/env bash
# Stand in for Cargo, resolving its output directory as Cargo does:
# `--target-dir` wins over `CARGO_TARGET_DIR`, which wins over `target`, and a
# `CARGO_BUILD_TARGET` triple adds a directory level.
set -euo pipefail

printf '%s\n' "$*" > "${NETSUKE_CARGO_ARGS}"
printf '%s\n' "${RUSTFLAGS-<unset>}" > "${NETSUKE_CARGO_RUSTFLAGS}"

target_dir="${CARGO_TARGET_DIR:-target}"
while (($#)); do
  if [[ "$1" == "--target-dir" ]]; then
    target_dir="$2"
    shift
  fi
  shift
done

output_dir="${target_dir}/${CARGO_BUILD_TARGET:+${CARGO_BUILD_TARGET}/}release"
mkdir -p "${output_dir}"
cat > "${output_dir}/${NETSUKE_FAKE_BINARY_NAME}" <<'BINARY'
#!/usr/bin/env bash
printf 'netsuke %s\n' "${NETSUKE_FAKE_VERSION}"
BINARY
chmod +x "${output_dir}/${NETSUKE_FAKE_BINARY_NAME}"
