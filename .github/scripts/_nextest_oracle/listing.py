"""Ask Nextest which test instances a selector matches, and read the sources.

`cargo nextest list` reports a test's selection through each testcase's
``filter-match.status``, not by omitting the tests it did not select, and its
exit code is 0 even when a selector matches nothing. Parsing the JSON is
therefore the only way to read the outcome; a status check would accept a
filter that silently selects no test at all, which is the defect being guarded.

The parameterized-instance prediction is read from the Rust sources rather than
from Nextest, so that the anchored selector is asserted to select exactly the
instances the source says exist. Two independent readings of one fact is what
makes the comparison evidence rather than a tautology.
"""

import json
import re
from pathlib import Path

from _nextest_oracle.runner import fail, run_command

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

#: Where the parameterized tests are declared, relative to the repository root.
TEST_SOURCE_ROOT = Path(__file__).resolve().parents[3] / "tests"


def masked(source: str) -> str:
    """Return ``source`` with comments and string literals blanked out.

    The UI fixtures hold Rust sources as string literals and comments describe
    attributes in prose, so a `#[case]` written in either would otherwise be
    counted as a real one and inflate the expected instance count.

    Returns
    -------
    str
        The source with both spans replaced by spaces of the same length.
    """
    blanked = LINE_COMMENT.sub(lambda match: " " * len(match.group(0)), source)
    return STRING_LITERAL.sub(lambda match: " " * len(match.group(0)), blanked)


def parameterized_tests() -> dict[str, int]:
    """Return each parameterized test to the number of its case instances.

    The count comes from the source rather than from Nextest, so it is an
    independent prediction: the anchored selector is then asserted to select
    exactly the instances the source says exist. A test with a `#[rstest]` but
    no `#[case]` is not parameterized -- it is one instance named plainly -- so
    only tests carrying at least one case attribute are returned.

    An empty corpus is refused rather than reported as success: without one
    parameterized test the assertion downstream would pass while checking
    nothing.

    Returns
    -------
    dict[str, int]
        Each parameterized test's name to its case count.
    """
    found: dict[str, int] = {}
    for path in sorted(TEST_SOURCE_ROOT.rglob("*.rs")):
        source = masked(path.read_text(encoding="utf-8"))
        for match in RUST_FUNCTION.finditer(source):
            cases = CASE_ATTRIBUTE.findall(match.group("attributes"))
            if cases:
                found[match.group("name")] = len(cases)
    if not found:
        fail(
            f"no parameterized test was found under {TEST_SOURCE_ROOT}, so "
            "the runtime check would assert nothing"
        )
    return found


def selected(env: dict[str, str], selector: str) -> set[str]:
    """Return the test instances ``selector`` selects, per Nextest itself.

    `--run-ignored all` is not optional here, and its absence is a silent
    failure of exactly the kind this package exists to catch. Without it,
    Nextest reports an ``#[ignore]``-gated test as ``mismatch`` whatever the
    filterset says, so a selector naming only ignored tests -- the mutation
    compile gate is one -- would be reported as selecting nothing. The flag is
    the same one `make test-kani-mutations` passes for the run itself, so the
    listing answers for the same set of tests the run would execute.

    Parameters
    ----------
    env
        The environment to run under, from `instrumented_environment`.
    selector
        One Nextest filter expression, replayed verbatim.

    Returns
    -------
    set[str]
        The names of the test instances the selector matched.
    """
    completed = run_command(
        [
            "cargo",
            "nextest",
            "list",
            "--all-features",
            "--all-targets",
            "--message-format",
            "json",
            "--run-ignored",
            "all",
            "--filterset",
            selector,
        ],
        env,
    )
    if completed.returncode != 0:
        fail(
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
