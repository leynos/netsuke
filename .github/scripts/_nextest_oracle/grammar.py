"""Read the filter expressions out of the Nextest configuration.

Two readings of the same file, and the difference between them is the point.
``configured_names`` extracts the *names* a filter in an accepted form names,
which the parameterized-instance check needs. ``all_filters`` returns every
``filter = '…'`` value verbatim, which is what a caller replays through Nextest
to learn whether the filter selects anything at all.

The verbatim read is deliberately independent of the extraction. A selector
re-synthesized from an extracted name would not round-trip a spelling the
grammar admits but writes differently -- ``test(/^NAME(::|$)/)`` transposes the
branches -- so a caller replaying the re-synthesized form would be checking a
filter that is not the one in the file.
"""

import re
from pathlib import Path

#: The repository root, three levels above this package's own directory.
REPO_ROOT = Path(__file__).resolve().parents[3]

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

#: An anchored selector, capturing the bare test name it names.
ANCHORED_SELECTOR_IN_CONFIG = re.compile(
    rf"test\(/\^{MODULE_PATH}(?P<name>[a-z0-9_]+)\(\$\|::\)/\)"
)

#: The rejected whole-name selector, capturing the bare test name it names. Read
#: as well as the anchored form so that a filter converted to this spelling is
#: reported rather than quietly leaving the check's scope: reading only the
#: anchored form would let the very defect this script exists for shrink the
#: set of tests it examines, and exit 0 having verified fewer of them.
LEGACY_SELECTOR_IN_CONFIG = re.compile(rf"test\(={MODULE_PATH}(?P<name>[a-z0-9_]+)\)")


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
