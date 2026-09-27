"""Hold the oracle's filter readers to the configuration they read.

`verify_nextest_anchored_filters.py` decides whether the Nextest filters in
`.config/nextest.toml` still name the tests they claim to. Every one of those
decisions starts in this package's `grammar` half: what a filter *is* comes
from `all_filters`, what each of its arms is comes from `filter_alternatives`,
and which test a selector names comes from `configured_names` and
`anchored_selectors`. The callers replay those readings through Nextest, so a
reading that is wrong does not fail -- it questions a filter that is not the one
in the file, or examines a subset and reports success.

That gap was live. Replacing the named capture group in
`ANCHORED_SELECTOR_IN_CONFIG` with two positional groups leaves all 25 tests
that previously touched this package passing, because none of them read the
grammar half at all. Each function here is exercised directly, over text, with
no file and no Nextest: these are pure functions of their input, and the
interesting behaviour is in the corner cases a configuration will eventually
contain rather than in the one file that happens to exist today.

The grammar this package reads is deliberately duplicated in the contracts,
which write the same rules from the other side of the same seam. A reader
narrower here than there does not fail; it goes blind to a spelling the
configuration is allowed to use. The transposed anchored suffix below is the
spelling where that had already happened.

Run via ``make test-workflow-contracts``.
"""

import sys
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
# The oracle lives beside the coverage-lane script it serves, outside any
# package the test tree can import by name. `verify_nextest_anchored_filters.py`
# resolves it the same way for the same reason; the path is inserted rather
# than the module copied, so the test exercises the code that actually runs.
sys.path.insert(0, str(REPO_ROOT / ".github" / "scripts"))

from _nextest_oracle import (  # ruff: ignore[module-import-not-at-top-of-file] - needs the sys.path insertion above.
    grammar,
)

#: A test declared inside a submodule, and the selector naming it. The selector
#: is 81 characters, too long to write inline at the indentation the assertions
#: use, so it is named once and the configuration is built from the constant.
#: Naming it also keeps the test and the text it writes from drifting apart.
MODULE_SCOPED_TEST = "every_patched_tree_compiles_under_denied_warnings"
MODULE_SCOPED_SELECTOR = f"test(/^compile_guard::{MODULE_SCOPED_TEST}($|::)/)"


@pytest.fixture
def config(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    """Point the module at a writable scratch configuration.

    `all_filters` reads the repository's real configuration, so every reading
    below would otherwise be a statement about one file's current contents. The
    readings are pure functions of the text, so the interesting cases are the
    spellings a configuration might carry rather than the ones it carries now.

    Parameters
    ----------
    tmp_path
        pytest-provided temporary directory.
    monkeypatch
        Used to rebind the module's configuration path for one test.

    Returns
    -------
    Path
        The scratch path, for a test that wants to write to it directly.
    """
    scratch = tmp_path / "nextest.toml"
    scratch.write_text("", encoding="utf-8")
    monkeypatch.setattr(grammar, "NEXTEST_CONFIG", scratch)
    return scratch


def write(config: Path, text: str) -> None:
    """Replace the scratch configuration's contents with ``text``."""
    config.write_text(text, encoding="utf-8")


def test_every_filter_line_is_returned_verbatim(config: Path) -> None:
    """Read each `filter = '…'` value exactly as written.

    Verbatim is the contract, not a convenience. The caller replays what it is
    given straight through Nextest, so a reading that normalised or
    re-synthesised the value would be checking a filter the file does not
    contain -- and a selector rebuilt from a name drops the module path, which
    is the one part a submodule test cannot do without.
    """
    write(
        config,
        "# a comment mentioning filter = 'ignored'\n"
        "filter = 'test(/^one($|::)/)'\n"
        'filter = "test(/^two($|::)/)"\n'
        "\tfilter = 'test(/^three($|::)/)'\n",
    )
    assert grammar.all_filters() == [
        "test(/^one($|::)/)",
        "test(/^two($|::)/)",
        "test(/^three($|::)/)",
    ], "every quoting and indentation must be read, and read verbatim"


def test_only_top_level_unions_are_split_into_alternatives(config: Path) -> None:
    """Split a union on its joining `|`, and leave a selector's own `|` alone.

    The character is overloaded: inside `test(...)` it is part of the regular
    expression, where `($|::)` means "end of name or a module separator". A
    split that ignored bracket depth would cut that in half and replay two
    fragments that are not selectors -- reporting an empty match as a fault in
    the configuration.
    """
    write(
        config,
        "filter = 'test(/^alpha($|::)/) | test(/^beta($|::)/)'\n"
        "filter = 'test(/^gamma(::|$)/)'\n"
        "filter = 'test(/^delta($|::)/) & test(/^epsilon($|::)/)'\n",
    )
    assert grammar.filter_alternatives() == [
        "test(/^alpha($|::)/)",
        "test(/^beta($|::)/)",
        "test(/^gamma(::|$)/)",
        "test(/^delta($|::)/) & test(/^epsilon($|::)/)",
    ], "a union's arms are replayed alone; a non-union filter stays whole"


def test_a_module_scoped_selector_names_its_bare_test(config: Path) -> None:
    """Drop the module path from the capture, and keep it in the match.

    The two halves answer different questions and must not be collapsed. Name
    comparisons are against the bare names the Rust sources yield, so a captured
    module path would report every module-scoped test as one this package has
    never seen. Replay needs the whole selector, because a test declared in a
    submodule is matched only while its module path is present.
    """
    write(config, f"filter = '{MODULE_SCOPED_SELECTOR}'\n")
    anchored, legacy = grammar.configured_names()
    assert anchored == {MODULE_SCOPED_TEST}, (
        "the module path must be matched but not captured, so the name group "
        "stays bare for the comparison against the Rust sources"
    )
    assert legacy == set(), "the anchored form is not the rejected whole-name form"
    assert grammar.anchored_selectors() == [
        (MODULE_SCOPED_SELECTOR, MODULE_SCOPED_TEST)
    ], "the selector is carried as written, with the bare name beside it"


def test_a_transposed_anchored_suffix_still_names_its_test(config: Path) -> None:
    """Read the anchored suffix in either order its branches may be written.

    `($|::)` and `(::|$)` denote the same set, and the contracts that admit the
    configuration admit both, so a reader that matched only the first would be
    narrower than the grammar it exists to read. That is this package's own
    failure mode in the reader: the filter is examined by nothing, the run exits
    0 having verified fewer tests than the configuration declares, and if the
    file used only this spelling the caller would refuse the run for holding no
    anchored filter at all -- accusing the configuration of a fault it does not
    have. Matching one spelling while `_top_level_alternatives` treats the two
    as one set also contradicts the module's own reading of the suffix.
    """
    write(
        config,
        "filter = 'test(/^alpha_default($|::)/)'\n"
        "filter = 'test(/^beta_default(::|$)/)'\n"
        "filter = 'test(/^gamma::delta_default(::|$)/)'\n",
    )
    anchored, legacy = grammar.configured_names()
    assert anchored == {"alpha_default", "beta_default", "delta_default"}, (
        "both spellings denote one grammar, so both must be read"
    )
    assert legacy == set(), "neither spelling is the rejected whole-name form"
    assert [name for _, name in grammar.anchored_selectors()] == [
        "alpha_default",
        "beta_default",
        "delta_default",
    ], "and each is still carried whole, in file order"
