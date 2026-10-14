"""The shared string walker sees every place YAML can hold a name.

Run via ``make test-workflow-contracts``.
"""

import pytest
from yaml_strings import iter_strings


@pytest.mark.parametrize(
    ("value", "expected"),
    [
        pytest.param({"CS_ACCESS_TOKEN": 1}, ["CS_ACCESS_TOKEN"], id="mapping-key"),
        pytest.param({"env": "x"}, ["env", "x"], id="key-before-value"),
        pytest.param([["a"], {"b": ["c"]}], ["a", "b", "c"], id="nested-lists"),
        pytest.param([None, 1, True], [], id="non-string-scalars"),
    ],
)
def test_the_walker_yields_keys_values_and_list_elements(
    value: object, expected: list[str]
) -> None:
    """Yield a mapping key as well as its value, in document order.

    A credential named only by a key, as an ``env`` entry names it, must
    reach every scan that reads through this walker.
    """
    found = list(iter_strings(value))
    assert found == expected, f"expected {expected!r} from {value!r}, got {found!r}"
