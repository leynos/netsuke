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
        {
            sub(/\r$/, "", $0)
            line = $0
            sub(/^ */, "", line)
            indentation = length($0) - length(line)
            if (indentation <= 3 && match(line, /^(```+|~~~+)/)) {
                delimiter = substr(line, 1, 1)
                width = RLENGTH
                suffix = substr(line, width + 1)
                if (!fenced) {
                    # Backtick fence info strings cannot contain backticks.
                    if (delimiter != "`" || index(suffix, "`") == 0) {
                        fenced = 1
                        fence_delimiter = delimiter
                        fence_width = width
                    }
                } else if (delimiter == fence_delimiter &&
                           width >= fence_width && suffix ~ /^[ \t]*$/) {
                    fenced = 0
                }
                next
            }
        }
        !fenced && indentation <= 3 && match(line, /^#+/) {
            heading = substr(line, 1, RLENGTH)
            suffix = substr(line, RLENGTH + 1)
            if (length(heading) <= 6 && (suffix == "" || suffix ~ /^[ \t]/)) {
                levels = levels separator heading
                separator = ","
                count++
            }
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
