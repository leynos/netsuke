#!/usr/bin/env bash
# Compare README heading counts and level order; translation meaning needs review.
# Run manually from any directory. This check is deliberately not a CI gate.
set -euo pipefail

cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
expected=''
status=0
for file in README.md README.de.md README.es.md README.fr.md \
    README.ja.md README.pt-BR.md README.zh-CN.md; do
    structure=$(awk '
        /^[[:space:]]*(```|~~~)/ { fenced = !fenced; next }
        !fenced && /^#{1,6} / {
            levels = levels separator $1
            separator = ","
            count++
        }
        END { printf "%d:%s\n", count, levels }
    ' "$file")
    printf '%s %s\n' "$file" "$structure"
    if [[ "$file" == README.md ]]; then
        expected=$structure
    elif [[ "$structure" != "$expected" ]]; then
        printf 'README heading structure differs: %s\n' "$file" >&2
        status=1
    fi
done
exit "$status"
