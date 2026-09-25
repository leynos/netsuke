"""Read the filter expressions out of the Nextest configuration.

Four readings of the same file, and the difference between them is the point.
``all_filters`` returns every ``filter = '…'`` value verbatim, which is what a
caller replays through Nextest to learn whether the filter selects anything at
all. ``filter_alternatives`` splits those values on their top-level ``|`` so
each arm of a union can be replayed on its own. ``anchored_selectors`` returns
each anchored selector *as written*, with the bare name it names, for a caller
that must replay the selector itself. ``configured_names`` extracts the names
in each accepted form, which the parameterized-instance check's comparisons
need, since those are keyed on the bare names the Rust sources yield.

The readings that replay are deliberately independent of the extraction. A
selector re-synthesized from an extracted name would not round-trip what the
file wrote: ``test(/^NAME(::|$)/)`` transposes the branches, and a selector for
a test declared in a submodule carries a ``module::`` prefix that the bare-name
reading drops entirely. Either way a caller replaying the re-synthesized form
would be checking a filter that is not the one in the file -- and, worse, would
report the resulting empty match as a fault in the configuration.
"""

import re
from pathlib import Path

#: The repository root, three levels above this package's own directory.
REPO_ROOT = Path(__file__).resolve().parents[3]

#: The Nextest configuration the filters are read from.
NEXTEST_CONFIG = REPO_ROOT / ".config" / "nextest.toml"

#: The selector the configuration must use. `test(=NAME)` compares the whole
#: name, so it selects no case instance of a parameterized test.
#:
#: This is the *writing* form, and it is not what a reader should replay. A
#: selector rebuilt from a name is missing whatever the file actually wrote --
#: the module path above all -- so a reader that re-synthesized from
#: `configured_names` would replay a filter the configuration does not contain
#: and report on that instead. Reading goes through `anchored_selectors`, which
#: returns each selector as written.
ANCHORED_SELECTOR = "test(/^{name}($|::)/)"

#: The selector this repository repaired away from, asserted to select nothing.
LEGACY_SELECTOR = "test(={name})"

#: A `filter = '…'` line in the Nextest configuration.
FILTER_LINE = re.compile(
    r"^\s*filter\s*=\s*(?P<quote>['\"])(?P<value>.*)(?P=quote)\s*$"
)

#: The leading `module::` segments of a test declared inside a submodule, which
#: Nextest prefixes onto the qualified name it matches a filter against. It is
#: deliberately not part of the capture group: the anchored names are
#: intersected with the parameterized tests read from the Rust sources, and
#: those are keyed by bare function name, so capturing the prefix would report
#: every module-scoped test as one this file has never seen. The contracts hold
#: the same rule for the same reason, and the two must agree -- a grammar wider
#: here than there reads a filter the configuration may not write, and a
#: narrower one is blind to a filter the contracts admit.
MODULE_PATH = r"(?:[a-z0-9_]+::)*"

#: An anchored selector, capturing the bare test name it names. The module path
#: is matched but deliberately outside the capture: the callers comparing names
#: key on the bare name the Rust sources yield, and capturing the prefix would
#: report every module-scoped test as one this package has never seen.
#:
#: The whole match is still available -- `finditer` exposes it as group 0 -- and
#: a reader that must *replay* a selector takes it, because a selector rebuilt
#: from the bare name would silently drop the module path the file wrote.
ANCHORED_SELECTOR_IN_CONFIG = re.compile(
    rf"test\(/\^{MODULE_PATH}(?P<name>[a-z0-9_]+)\(\$\|::\)/\)"
)

#: The rejected whole-name selector, capturing the bare test name it names. Read
#: as well as the anchored form so that a filter converted to this spelling is
#: reported rather than quietly leaving the check's scope: reading only the
#: anchored form would let the very defect this script exists for shrink the
#: set of tests it examines, and exit 0 having verified fewer of them.
LEGACY_SELECTOR_IN_CONFIG = re.compile(rf"test\(={MODULE_PATH}(?P<name>[a-z0-9_]+)\)")


def _top_level_alternatives(value: str) -> list[str]:
    """Split one filter on the ``|`` operators that join its alternatives.

    Only a ``|`` outside every bracket separates alternatives. The character is
    overloaded in this grammar -- inside a ``test(...)`` argument it is part of
    the regular expression, where ``($|::)`` means "end of name or a module
    separator", and ``(::|$)`` spells the same set transposed -- so a naive
    split would cut those in half and replay fragments that are not selectors at
    all. Tracking bracket depth keeps the regular expressions whole.

    This exists so an alternative can be judged on its own. A union that
    selected something has not shown that *each* of its arms does: one live arm
    satisfies the whole expression while a dead one beside it leaves a test
    running under the defaults, which is the failure this package exists to
    catch, and it is invisible until the arms are replayed separately.

    Parameters
    ----------
    value : str
        One ``filter = '…'`` value.

    Returns
    -------
    list[str]
        The alternatives, stripped, each replayed on its own.
    """
    parts: list[str] = []
    depth = 0
    current: list[str] = []
    for character in value:
        if character in "([{":
            depth += 1
        elif character in ")]}":
            depth = max(0, depth - 1)
        elif character == "|" and depth == 0:
            parts.append("".join(current))
            current = []
            continue
        current.append(character)
    parts.append("".join(current))
    return [part.strip() for part in parts if part.strip()]


def filter_alternatives() -> list[str]:
    """Return every filter alternative the configuration declares.

    One filter may name several tests by joining selectors with ``|``. Each
    alternative is returned separately, because a union is satisfied by any one
    of its arms: a filter whose first selector matches its test still passes
    while a second selector matches nothing, and the test that second selector
    was meant to cover runs under the defaults. Replaying the alternatives
    individually is what makes that visible.

    Anything that is not a top-level union -- a single selector, or a filter
    built from intersection, subtraction, or negation -- comes back whole and is
    replayed exactly as written.

    Returns
    -------
    list[str]
        The filter alternatives, in file order.
    """
    return [
        alternative
        for value in all_filters()
        for alternative in _top_level_alternatives(value)
    ]


def all_filters() -> list[str]:
    """Return every ``filter = '…'`` value in the configuration, verbatim.

    Whatever form each filter is written in, and whichever override carries
    it: a `test-group` assignment and a widened `slow-timeout` both select
    their tests through this field, and both fail the same way when it selects
    nothing. No name is extracted and no grammar is applied, so a spelling this
    package has not reviewed is still replayed rather than skipped.

    Returns
    -------
    list[str]
        The filter expression of every line carrying one, in file order.
    """
    text = NEXTEST_CONFIG.read_text(encoding="utf-8")
    return [
        match.group("value")
        for line in text.splitlines()
        if (match := FILTER_LINE.match(line)) is not None
    ]


def anchored_selectors() -> list[tuple[str, str]]:
    """Return each anchored selector as written, with the bare name it names.

    The whole-match half is the point. ``configured_names`` reduces a selector
    to the bare name its comparisons need, and a caller that rebuilt a selector
    from that name would replay one the configuration does not contain: the
    module path is exactly what the bare-name reading drops, and a test declared
    in a submodule is matched only when its path is present. The reader that
    replays a filter therefore takes the selector verbatim from here.

    A filter may carry several selectors, so each is returned separately rather
    than one per line: a union whose alternatives are only checked together can
    hide a dead one behind a live one, which is the defect the caller exists to
    catch.

    Returns
    -------
    list[tuple[str, str]]
        Each anchored selector as written and the bare test name it names, in
        file order.
    """
    found: list[tuple[str, str]] = []
    for value in all_filters():
        found.extend(
            (match.group(0), match.group("name"))
            for match in ANCHORED_SELECTOR_IN_CONFIG.finditer(value)
        )
    return found


def configured_names() -> tuple[set[str], set[str]]:
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
    anchored: set[str] = set()
    legacy: set[str] = set()
    for value in all_filters():
        anchored.update(ANCHORED_SELECTOR_IN_CONFIG.findall(value))
        legacy.update(LEGACY_SELECTOR_IN_CONFIG.findall(value))
    return anchored, legacy
