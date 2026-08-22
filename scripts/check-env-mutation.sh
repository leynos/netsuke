#!/usr/bin/env sh
# Reject in-process environment mutation anywhere in the source tree.
#
# These std::env free functions mutate process-global state that parallel test
# execution cannot isolate. Behaviour that depends on them must accept injected
# data instead; child-process configuration is confined to the Command builders
# (Command::env, Command::env_clear, Command::current_dir), which this gate
# deliberately does not match.
set -eu

root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"

# Only the std::env:: free functions are rejected: the pattern is anchored to
# the full `std::env::` path, so `Command::env` and `Command::current_dir`
# builder calls never match.
if grep -RInE --include='*.rs' 'std::env::(set_var|remove_var|set_current_dir)' \
    "$root/src" "$root/tests" "$root/test_support"; then
    echo 'error: in-process environment mutation is forbidden (see AGENTS.md testing mandate)' >&2
    exit 1
fi
