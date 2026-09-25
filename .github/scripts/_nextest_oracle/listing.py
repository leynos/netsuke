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

#: The opening of a raw string literal, with its `#` fence length.
RAW_STRING_START = re.compile(r'(?:br|r)(?P<hashes>#*)"')

#: Where the parameterized tests are declared, relative to the repository root.
TEST_SOURCE_ROOT = Path(__file__).resolve().parents[3] / "tests"


def _line_comment_end(source: str, index: int) -> int | None:
    """Return the end of a line comment beginning at ``index``."""
    if not source.startswith("//", index):
        return None
    end = source.find("\n", index)
    return len(source) if end < 0 else end


def _block_comment_end(source: str, index: int) -> int | None:
    """Return the end of a possibly nested block comment at ``index``."""
    if not source.startswith("/*", index):
        return None
    depth, end = 1, index + 2
    while depth and end < len(source):
        if source.startswith("/*", end):
            depth, end = depth + 1, end + 2
        elif source.startswith("*/", end):
            depth, end = depth - 1, end + 2
        else:
            end += 1
    return end


def _raw_string_end(source: str, index: int) -> int | None:
    """Return the end of a raw string literal beginning at ``index``."""
    match = RAW_STRING_START.match(source, index)
    if match is None:
        return None
    terminator = f'"{match.group("hashes")}'
    end = source.find(terminator, match.end())
    return len(source) if end < 0 else end + len(terminator)


def _string_literal_end(source: str, index: int) -> int | None:
    """Return the end of a string literal beginning at ``index``."""
    if source[index] != '"':
        return None
    end = index + 1
    while end < len(source):
        match source[end]:
            case "\\":
                end += 2
            case '"':
                return end + 1
            case _:
                end += 1
    return len(source)


def _character_literal_end(source: str, index: int) -> int | None:
    """Return the end of a character literal, refusing lifetimes and labels."""
    if source[index] != "'":
        return None
    end = index + 1
    if source.startswith("\\u{", end):
        closing_brace = source.find("}", end + 3)
        end = len(source) if closing_brace < 0 else closing_brace + 1
    elif end < len(source) and source[end] == "\\":
        end += 2
    else:
        end += 1
    return end + 1 if end < len(source) and source[end] == "'" else None


def _non_code_end(source: str, index: int) -> int | None:
    """Return the end of non-code text beginning at ``index``, when present."""
    for boundary in (_line_comment_end, _block_comment_end, _raw_string_end):
        if (end := boundary(source, index)) is not None:
            return end
    if (end := _string_literal_end(source, index)) is not None:
        return end
    return _character_literal_end(source, index)


def masked(source: str) -> str:
    """Return ``source`` with comments and string literals blanked out.

    The UI fixtures hold Rust sources as string literals and comments describe
    attributes in prose, so a `#[case]` written in either would otherwise be
    counted as a real one and inflate the expected instance count.

    This scans left to right and consumes each span it finds, rather than
    applying one regular expression per construct over the whole file. The
    difference is not stylistic. Two global passes cannot know which of them is
    *inside* the other, so whichever runs first corrupts the second: masking
    comments first makes a literal such as ``"// mod wired"`` lose its closing
    quote and blank the rest of its line, while masking literals first makes a
    comment containing an unmatched quote swallow the code after it. Advancing
    one position at a time removes the question, because a string and a comment
    never begin at the same character -- only one of them can be consumed, and
    the scan resumes after it.

    Raw strings are tried before plain literals so the terminator's fence
    length is honoured; `tests/` carries them in bulk, and reading one as a
    plain literal would resume mid-string and expose fixture text as code.

    Returns
    -------
    str
        The source with every non-code span replaced by spaces of the same
        length, so line and column numbers are unchanged.
    """
    blanked = list(source)
    index = 0
    while index < len(source):
        end = _non_code_end(source, index)
        if end is None:
            index += 1
            continue
        blanked[index:end] = " " * (end - index)
        index = end
    return "".join(blanked)


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
    # Reaching here means `cargo nextest list` exited 0, which it does having
    # listed *some* suite. A document without the key is therefore not an empty
    # corpus but a listing this function failed to read, and returning an empty
    # set for it would report "selects nothing" for every selector -- the
    # failure this module exists to catch, reported against innocent filters.
    # The empty case above already refuses a reading that found nothing; this
    # refuses a reading that could not be performed.
    if "rust-suites" not in document:
        fail(
            "`cargo nextest list` produced no `rust-suites` key, so the "
            f"listing for {selector!r} could not be read; its keys were "
            f"{sorted(document)}"
        )
    return {
        name
        for suite in document["rust-suites"].values()
        for name, case in suite.get("testcases", {}).items()
        if case.get("filter-match", {}).get("status") == "matches"
    }
