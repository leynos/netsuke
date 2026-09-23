#!/usr/bin/env python3
"""Prove the anchored Nextest filters select their tests at run time.

`.config/nextest.toml` applies a policy to a test by evaluating an override's
filter against real test names. Parsing the configuration cannot tell whether a
filter selects anything, and the contract tests that read the file can only
hold its *writing* to the anchored grammar. This script closes the gap at the
other end, by asking Nextest itself. It asserts two things:

  - every filter expression in the configuration, replayed verbatim, selects at
    least one test. This is the whole-file check, and it is deliberately
    independent of grammar and of names: a filter for a root-level test, for a
    parameterized test's cases, or for a test declared in a submodule is judged
    the same way, and so is a spelling this file has not reviewed. A filter
    that selects nothing is the defect -- it leaves its test running under the
    defaults while the policy still looks enforced.
  - each anchored filter naming a parameterized test selects exactly that
    test's case instances and nothing else, while the rejected whole-name
    `test(=NAME)` form selects none of them.

The second check alone was scoped past a whole class, which is why the first
exists: it only ever examined filters naming a parameterized test, so a filter
naming a plain test in a submodule -- whose qualified name carries a `module::`
prefix the `^` anchor stops short of -- was never replayed at all.

It is deliberately not a workflow-contract test. The check needs compiled test
binaries, and the `Workflow contract tests` lane runs before the first Rust
build and must remain static. It runs on the coverage lane instead, after `Test
and Measure Coverage`, and reuses that step's instrumented build tree rather
than compiling. The reading itself lives in the `_nextest_oracle` package
beside this file, so that each unit stays under the module ceiling.

Usage:
    python .github/scripts/verify_nextest_anchored_filters.py
"""

import argparse
import re
import sys
from pathlib import Path

# The workflow runs this file by path, from the repository root, so the
# package beside it is not importable by default. `lint-workflow-scripts`
# loads it through `runpy`, where `sys.path[0]` is the empty string -- the
# current directory at import time, not this file's directory -- so relying on
# the ambient path would resolve the import in one of the two callers and not
# the other. Doing it explicitly here makes both work.
sys.path.insert(0, str(Path(__file__).resolve().parent))

from _nextest_oracle import (
    ANCHORED_SELECTOR,
    LEGACY_SELECTOR,
    all_filters,
    configured_names,
    fail,
    instrumented_environment,
    parameterized_tests,
    selected,
)


def _check(env: dict[str, str], name: str, cases: int) -> None:
    """Assert both selector forms behave as the configuration assumes.

    The anchored form must select one instance per case the source declares,
    and nothing outside the test's own namespace. An rstest appends the case's
    name when it has one, so the instance is ``name::case_1_slug`` rather than
    ``name::case_1``; the assertion is therefore on the instance's prefix, with
    the case count supplying the number of instances expected. A selector that
    matched nothing fails, and so does one that reached past the test -- which
    is what separates an anchored selector from the unanchored ``~`` form.

    The instance prefix tolerates a leading module path. A parameterized test
    declared in a submodule is named ``module::name::case_1`` at run time, so
    anchoring at the bare name would report such a test's own instances as
    strays -- the same module-qualification blind spot the whole-file replay
    above exists to catch, in the one place a bare name is still assumed.

    The legacy form must select nothing: ``test(=NAME)`` compares the whole
    name, so it cannot match ``NAME::case_…`` at all. Its exit status is 0
    whether or not it selected anything, so only the parsed names can tell.
    """
    anchored = ANCHORED_SELECTOR.format(name=name)
    matched = selected(env, anchored)
    instance = re.compile(
        rf"^(?:[a-z0-9_]+::)*{re.escape(name)}::case_(?P<index>\d+)(?:_|$)"
    )
    matches = [(found, instance.match(found)) for found in matched]
    stray = sorted(found for found, match in matches if match is None)
    found_indices = {int(match["index"]) for _, match in matches if match is not None}
    missing = sorted(set(range(1, cases + 1)) - found_indices)
    if stray or missing:
        command = f"cargo nextest list --filterset {anchored}"
        fail(
            f"{anchored!r} must match one instance per `#[case]` of {name} "
            f"({cases} declared) and nothing else; it matched "
            f"{sorted(matched)!r}, leaving case(s) {missing!r} unmatched and "
            f"reaching {stray!r}, which the test does not declare. Command: "
            f"`{command}`"
        )
    legacy = LEGACY_SELECTOR.format(name=name)
    over_selected = selected(env, legacy)
    if over_selected:
        command = f"cargo nextest list --filterset {legacy}"
        fail(
            f"{legacy!r} must match no instance of {name}, because the "
            f"whole-name form cannot match `{name}::case_1`; it matched "
            f"{sorted(over_selected)!r}. Command: `{command}`"
        )
    print(f"ok: {anchored} matches {sorted(matched)}; {legacy} matches nothing")


def _check_every_filter_selects_something(env: dict[str, str]) -> None:
    """Replay every filter in the configuration and require a non-empty result.

    Each value is replayed as written, with no name extracted and no grammar
    applied, so a filter written in a spelling this file has not reviewed is
    still held to the one property that matters: it must select a test. The
    command that reproduces a failure is named in the message, because a filter
    that selects nothing looks exactly like one that works. A configuration
    declaring no filter at all is refused too: an empty corpus would leave this
    check passing while asserting nothing.

    Any filter that selects no test ends the run through `fail`, so the exit is
    non-zero and names the filter.
    """
    filters = all_filters()
    if not filters:
        fail(
            "the Nextest configuration declares no `filter = '…'`, so there is "
            "nothing to replay; a configuration whose filters cannot be read is "
            "not the same as one whose filters all select a test"
        )
    for filter_ in filters:
        if not selected(env, filter_):
            command = f"cargo nextest list --run-ignored all --filterset {filter_}"
            fail(
                f"the filter {filter_!r} selects no test, so the policy it "
                f"carries applies to nothing and the test it names runs under "
                f"the defaults. Reproduce with `{command}`"
            )
    print(f"replayed {len(filters)} filter(s); each selects at least one test")


def main(argv: list[str] | None = None) -> int:
    """Assert every filter in the configuration selects the tests it names.

    Parameters
    ----------
    argv
        Command-line arguments; defaults to ``sys.argv[1:]``.

    Returns
    -------
    int
        ``0`` when every filter selects a test, every anchored filter naming a
        parameterized test selects its instances, and the legacy exact form
        selects none of them; ``1`` in every other case.
    """
    argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    ).parse_args(argv)
    parameterized = parameterized_tests()
    anchored, legacy = configured_names()
    legacy_parameterized = sorted(set(parameterized) & legacy)
    if legacy_parameterized:
        fail(
            f"the configuration filters {legacy_parameterized!r} with the "
            f"whole-name `test(=NAME)` form, which selects no case instance of "
            f"a parameterized test and leaves it outside its policy; use "
            f"`test(/^NAME($|::)/)`"
        )
    filtered = sorted(set(parameterized) & anchored)
    if not filtered:
        fail(
            f"no anchored filter names a parameterized test; the anchored "
            f"filters name {sorted(anchored)!r} and the parameterized tests "
            f"are {sorted(parameterized)!r}. The runtime check exists to hold "
            f"a filtered parameterized test to its case instances, so there is "
            f"nothing here to verify"
        )
    env = instrumented_environment()
    _check_every_filter_selects_something(env)
    for name in filtered:
        _check(env, name, parameterized[name])
    print(f"verified {len(filtered)} filtered parameterized test(s)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
