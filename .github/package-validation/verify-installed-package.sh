#!/bin/sh

set -eu

if [ "$#" -lt 1 ] || [ -z "$1" ]; then
    printf 'usage: %s PACKAGE-NAME\n' "$0" >&2
    exit 2
fi

package_name=$1

if ! "$package_name" --version; then
    printf 'FAIL: %s --version failed\n' "$package_name" >&2
    exit 1
fi
printf 'PASS: %s --version\n' "$package_name"

if ! man_page=$(man -w "$package_name"); then
    printf 'FAIL: man -w %s did not resolve a manual page\n' "$package_name" >&2
    exit 1
fi
if [ -z "$man_page" ]; then
    printf 'FAIL: man -w %s returned an empty path\n' "$package_name" >&2
    exit 1
fi
printf 'PASS: man -w %s resolved to %s\n' "$package_name" "$man_page"

if command -v dpkg-query >/dev/null 2>&1; then
    if ! dependencies=$(dpkg-query -W -f='${Depends}\n' "$package_name"); then
        printf 'FAIL: could not read Debian dependencies for %s\n' "$package_name" >&2
        exit 1
    fi
    if ! printf '%s\n' "$dependencies" | grep -Eq '(^|,)[[:space:]]*ninja-build(:[[:alnum:]][[:alnum:].+-]*)?([[:space:]]+\([^)]*\))?[[:space:]]*(,|$)'; then
        printf 'FAIL: Debian package %s does not declare ninja-build\n' "$package_name" >&2
        exit 1
    fi
    printf 'PASS: Debian package %s declares ninja-build\n' "$package_name"
elif command -v rpm >/dev/null 2>&1; then
    if ! requirements=$(rpm -q --requires "$package_name"); then
        printf 'FAIL: could not read RPM requirements for %s\n' "$package_name" >&2
        exit 1
    fi
    if ! printf '%s\n' "$requirements" | grep -Eq '^ninja-build([[:space:]]|$)'; then
        printf 'FAIL: RPM package %s does not declare ninja-build\n' "$package_name" >&2
        exit 1
    fi
    printf 'PASS: RPM package %s declares ninja-build\n' "$package_name"
else
    printf 'FAIL: neither dpkg-query nor rpm is available to inspect dependencies\n' >&2
    exit 1
fi
