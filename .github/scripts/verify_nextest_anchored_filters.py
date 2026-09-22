#!/usr/bin/env python3
"""Prove the anchored Nextest filters select their instances at run time.

`.config/nextest.toml` filters the `nested-cargo-builds` group with
``test(/^NAME($|::)/)`` rather than ``test(=NAME)``, because Nextest's ``=``
form compares a whole test name and a parameterized ``#[rstest]`` is listed as
``NAME::case_1_…``. A filter written in the rejected form therefore selects
none of a parameterized test's instances while still parsing cleanly, so the
test silently runs under the default policy.

Parsing the configuration cannot see that: both spellings are well-formed, and
the contract tests that read the file can only hold the *writing* to the
anchored grammar. This script closes the gap at the other end, by asking
Nextest itself which tests each form selects. It asserts that the anchored form
selects the case instances a filtered parameterized test must resolve to, and
that the legacy exact form selects none of them.

It is deliberately not a workflow-contract test. The check needs compiled test
binaries, and the `Workflow contract tests` lane runs before the first Rust
build and must remain static. It runs on the coverage lane instead, after `Test
and Measure Coverage`, and reuses that step's instrumented build tree rather
than compiling: `cargo nextest list` builds, so a second build here would both
duplicate the work the coverage run already did and repeat the flags that run
established. The tree is reused by asking `cargo llvm-cov show-env` for the
environment the coverage action used -- the same `RUSTFLAGS` and the same
`llvm-cov-target` directory -- and exporting it before listing.

Usage:
    python .github/scripts/verify_nextest_anchored_filters.py
"""

import argparse
import json
import os
import re
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the cargo boundary is this script's job.
import sys
from pathlib import Path

#: The repository root, two levels above this script's own directory.
REPO_ROOT = Path(__file__).resolve().parents[2]

#: The Nextest configuration the filters are read from.
NEXTEST_CONFIG = REPO_ROOT / ".config" / "nextest.toml"

#: The selector the configuration must use. `test(=NAME)` compares the whole
#: name, so it selects no case instance of a parameterized test.
ANCHORED_SELECTOR = "test(/^{name}($|::)/)"

#: The selector this repository repaired away from, asserted to select nothing.
LEGACY_SELECTOR = "test(={name})"

#: A `filter = '…'` line in the Nextest configuration.
FILTER_LINE = re.compile(
    r"^\s*filter\s*=\s*(?P<quote>['\"])(?P<value>.*)(?P=quote)\s*$"
)

#: One `export NAME=value` line of `cargo llvm-cov show-env --export-prefix`
#: output. The value is single-quoted only when it needs to be, so both
#: spellings are accepted; requiring the quotes silently drops the unquoted
#: lines, which are most of them.
EXPORTED_VARIABLE = re.compile(
    r"^export (?P<name>[A-Za-z_][A-Za-z0-9_]*)="
    r"(?:'(?P<single>.*)'|(?P<bare>\S*))$"
)

#: An anchored selector, capturing the test name it names.
ANCHORED_SELECTOR_IN_CONFIG = re.compile(r"test\(/\^(?P<name>[a-z0-9_]+)\(\$\|::\)/\)")

#: The rejected whole-name selector, capturing the test name it names. Read as
#: well as the anchored form so that a filter converted to this spelling is
#: reported rather than quietly leaving the check's scope: reading only the
#: anchored form would let the very defect this script exists for shrink the
#: set of tests it examines, and exit 0 having verified fewer of them.
LEGACY_SELECTOR_IN_CONFIG = re.compile(r"test\(=(?P<name>[a-z0-9_]+)\)")

#: A Rust function with the attribute block that precedes it. This is the
#: pattern `tests/workflow_contracts/nextest_rust_test_discovery.py` uses to
#: classify tests; it is copied rather than imported because this script runs
#: on the coverage lane and must not depend on the test tree. `#\[[^\n]*\]`
#: takes the single-line `#[case::name(args)]` spelling, and the alternation
#: takes the multi-line one by running to the first line that closes at the
#: same indent.
RUST_FUNCTION = re.compile(
    r"(?ms)^(?P<indent>[ \t]*)"
    r"(?P<attributes>(?:(?P=indent)#\[[^\n]*\]\s*|"
    r"(?P=indent)#\[[\s\S]*?^(?P=indent)[^\n]*\]\s*)*)"
    r"(?P<signature>(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?"
    r"fn\s+(?P<name>[a-z0-9_]+)\b[^\{]*)\{"
)

#: A `#[case]` attribute, named or positional, in either bracket spelling. A
#: `#[case]` *argument* in a signature has a `]` where this requires `(` or
#: `[`, so only attributes are counted.
CASE_ATTRIBUTE = re.compile(r"#\[case(?:::[a-z0-9_]+)?[\(\[]")

#: A string literal, so its contents can be masked before the scan.
STRING_LITERAL = re.compile(r'"(?:[^"\\]|\\.)*"')

#: A line comment, so its contents can be masked before the scan.
LINE_COMMENT = re.compile(r"//[^\n]*")

#: Where the parameterized tests are declared.
TEST_SOURCE_ROOT = REPO_ROOT / "tests"

#: The environment variable holding the instrumented build tree.
LLVM_COV_TARGET_DIR = "CARGO_LLVM_COV_TARGET_DIR"


def _fail(message: str) -> None:
    """Report ``message`` on stderr and exit non-zero."""
    print(f"::error title=Nextest anchored filters::{message}", file=sys.stderr)
    raise SystemExit(1)


def _run(argv: list[str], env: dict[str, str]) -> subprocess.CompletedProcess[str]:
    """Run ``argv`` and return the completed process, echoing what ran."""
    print(f"+ {' '.join(argv)}", flush=True)
    return subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - a fixed argument list, never a shell.
        argv, capture_output=True, check=False, env=env, shell=False, text=True
    )


def _instrumented_environment() -> dict[str, str]:
    """Return the environment of the coverage step's instrumented build tree.

    `cargo llvm-cov show-env --export-prefix` prints the coverage action's own
    environment as shell assignments. Exporting them and then selecting the
    coverage target directory puts this step's build in the tree the coverage
    run already populated, so `cargo nextest list` reuses it instead of
    compiling. The ambient `RUSTFLAGS` is preserved by the command and joined
    with the instrumentation flags, which is what keeps this step's fingerprint
    equal to the coverage run's.

    A `cargo llvm-cov` that is absent, or that reports no target directory, is
    refused by name rather than allowed to proceed. That is the one condition
    under which reuse quietly becomes a second build, and a build that happens
    to succeed on a warm cache would hide it.

    Returns
    -------
    dict[str, str]
        The environment to run `cargo nextest list` under.
    """
    completed = _run(
        ["cargo", "llvm-cov", "show-env", "--export-prefix"], dict(os.environ)
    )
    if completed.returncode != 0:
        _fail(
            "`cargo llvm-cov show-env` failed, so the instrumented build tree "
            f"cannot be reused and this step would become a second build: "
            f"{completed.stderr.strip()}"
        )
    env = dict(os.environ)
    for line in completed.stdout.splitlines():
        match = EXPORTED_VARIABLE.match(line)
        if match is not None:
            value = (
                match.group("single")
                if match.group("single") is not None
                else match.group("bare")
            )
            env[match.group("name")] = value
    target_dir = env.get(LLVM_COV_TARGET_DIR)
    if not target_dir:
        _fail(
            f"`cargo llvm-cov show-env` reported no {LLVM_COV_TARGET_DIR}, so "
            "there is no instrumented tree to reuse"
        )
    env["CARGO_TARGET_DIR"] = f"{target_dir}/llvm-cov-target"
    return env


def _masked(source: str) -> str:
    """Return ``source`` with comments and string literals blanked out.

    The UI fixtures hold Rust sources as string literals and comments describe
    attributes in prose, so a `#[case]` written in either would otherwise be
    counted as a real one and inflate the expected instance count.

    Returns
    -------
    str
        The source with both spans replaced by spaces of the same length.
    """
    masked = LINE_COMMENT.sub(lambda match: " " * len(match.group(0)), source)
    return STRING_LITERAL.sub(lambda match: " " * len(match.group(0)), masked)


def _parameterized_tests() -> dict[str, int]:
    """Return each parameterized test to the number of its case instances.

    The count comes from the source rather than from Nextest, so it is an
    independent prediction: the anchored selector is then asserted to select
    exactly the instances the source says exist. A test with a `#[rstest]` but
    no `#[case]` is not parameterized -- it is one instance named plainly -- so
    only tests carrying at least one case attribute are returned.

    An empty corpus is refused rather than reported as success: without one
    parameterized test the assertion below would pass while checking nothing.

    Returns
    -------
    dict[str, int]
        Each parameterized test's name to its case count.
    """
    found: dict[str, int] = {}
    for path in sorted(TEST_SOURCE_ROOT.rglob("*.rs")):
        source = _masked(path.read_text(encoding="utf-8"))
        for match in RUST_FUNCTION.finditer(source):
            attributes = match.group("attributes")
            cases = CASE_ATTRIBUTE.findall(attributes)
            if cases:
                found[match.group("name")] = len(cases)
    if not found:
        _fail(
            f"no parameterized test was found under {TEST_SOURCE_ROOT}, so "
            "the runtime check would assert nothing"
        )
    return found


def _configured_names() -> tuple[set[str], set[str]]:
    """Return the names the anchored and the legacy filters in the config name.

    Both spellings are read. Reading only the anchored form would let a filter
    repaired *into* the legacy form drop out of the set this script verifies,
    so the run would report success having checked fewer tests than before --
    the failure mode the script exists to prevent, wearing its own shape.

    Returns
    -------
    tuple[set[str], set[str]]
        The names the anchored filters select, and the names written in the
        rejected whole-name form.
    """
    text = NEXTEST_CONFIG.read_text(encoding="utf-8")
    anchored: set[str] = set()
    legacy: set[str] = set()
    for line in text.splitlines():
        match = FILTER_LINE.match(line)
        if match is not None:
            value = match.group("value")
            anchored.update(ANCHORED_SELECTOR_IN_CONFIG.findall(value))
            legacy.update(LEGACY_SELECTOR_IN_CONFIG.findall(value))
    return anchored, legacy


def _selected(env: dict[str, str], selector: str) -> set[str]:
    """Return the test instances ``selector`` selects, per Nextest itself.

    `cargo nextest list` reports a test's selection through each testcase's
    `filter-match.status`, not by omitting unselected tests, and its exit code
    is 0 even when a selector matches nothing. Parsing the JSON is therefore
    the only way to read the outcome; a status check would accept a filter that
    silently selects no test at all, which is the defect being guarded.

    Parameters
    ----------
    env
        The environment to run under, from `_instrumented_environment`.
    selector
        One Nextest filter expression.

    Returns
    -------
    set[str]
        The names of the test instances the selector matched.
    """
    completed = _run(
        [
            "cargo",
            "nextest",
            "list",
            "--all-features",
            "--all-targets",
            "--message-format",
            "json",
            "--filterset",
            selector,
        ],
        env,
    )
    if completed.returncode != 0:
        _fail(
            f"`cargo nextest list` failed for selector {selector!r} (exit "
            f"{completed.returncode}): {completed.stderr.strip()}"
        )
    document = json.loads(completed.stdout)
    return {
        name
        for suite in document.get("rust-suites", {}).values()
        for name, case in suite.get("testcases", {}).items()
        if case.get("filter-match", {}).get("status") == "matches"
    }


def _check(env: dict[str, str], name: str, cases: int) -> None:
    """Assert both selector forms behave as the configuration assumes.

    The anchored form must select one instance per case the source declares,
    and nothing outside the test's own namespace. An rstest appends the case's
    name when it has one, so the instance is ``name::case_1_slug`` rather than
    ``name::case_1``; the assertion is therefore on the instance's prefix, with
    the case count supplying the number of instances expected. A selector that
    matched nothing fails, and so does one that reached past the test -- which
    is what separates an anchored selector from the unanchored ``~`` form.

    The legacy form must select nothing: ``test(=NAME)`` compares the whole
    name, so it cannot match ``NAME::case_…`` at all. Its exit status is 0
    whether or not it selected anything, so only the parsed names can tell.
    """
    anchored = ANCHORED_SELECTOR.format(name=name)
    matched = _selected(env, anchored)
    instance = re.compile(rf"^{re.escape(name)}::case_(?P<index>\d+)(?:_|$)")
    matches = [(selected, instance.match(selected)) for selected in matched]
    stray = sorted(selected for selected, match in matches if match is None)
    found = {int(match["index"]) for _, match in matches if match is not None}
    missing = sorted(set(range(1, cases + 1)) - found)
    if stray or missing:
        command = f"cargo nextest list --filterset {anchored}"
        _fail(
            f"{anchored!r} must match one instance per `#[case]` of {name} "
            f"({cases} declared) and nothing else; it matched "
            f"{sorted(matched)!r}, leaving case(s) {missing!r} unmatched and "
            f"reaching {stray!r}, which the test does not declare. Command: "
            f"`{command}`"
        )
    legacy = LEGACY_SELECTOR.format(name=name)
    over_selected = _selected(env, legacy)
    if over_selected:
        command = f"cargo nextest list --filterset {legacy}"
        _fail(
            f"{legacy!r} must match no instance of {name}, because the "
            f"whole-name form cannot match `{name}::case_1`; it matched "
            f"{sorted(over_selected)!r}. Command: `{command}`"
        )
    print(f"ok: {anchored} matches {sorted(matched)}; {legacy} matches nothing")


def main(argv: list[str] | None = None) -> int:
    """Assert every anchored filter selects the instances it names.

    Parameters
    ----------
    argv
        Command-line arguments; defaults to ``sys.argv[1:]``.

    Returns
    -------
    int
        ``0`` when every anchored filter selects its instances and the legacy
        exact form selects none of them, ``1`` in every other case.
    """
    argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    ).parse_args(argv)
    parameterized = _parameterized_tests()
    anchored, legacy = _configured_names()
    legacy_parameterized = sorted(set(parameterized) & legacy)
    if legacy_parameterized:
        _fail(
            f"the configuration filters {legacy_parameterized!r} with the "
            f"whole-name `test(=NAME)` form, which selects no case instance of "
            f"a parameterized test and leaves it outside its policy; use "
            f"`test(/^NAME($|::)/)`"
        )
    filtered = sorted(set(parameterized) & anchored)
    if not filtered:
        _fail(
            f"no anchored filter names a parameterized test; the anchored "
            f"filters name {sorted(anchored)!r} and the parameterized tests "
            f"are {sorted(parameterized)!r}. The runtime check exists to hold "
            f"a filtered parameterized test to its case instances, so there is "
            f"nothing here to verify"
        )
    env = _instrumented_environment()
    for name in filtered:
        _check(env, name, parameterized[name])
    print(f"verified {len(filtered)} filtered parameterized test(s)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
