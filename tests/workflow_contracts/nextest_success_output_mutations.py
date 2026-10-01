"""Build one-condition mutations of the RFC count's Nextest override.

Each fixture includes an unrelated override, so deleting the required override
exercises selection rather than an empty document. The contract loads these
fixtures through the existing TOML reader.
"""

import typing as typ

from nextest_success_output import IMMEDIATE_OUTPUT_TESTS

GUARDED_TEST: typ.Final[str] = IMMEDIATE_OUTPUT_TESTS[0]
_SELECTOR: typ.Final[str] = f"test(/^{GUARDED_TEST}($|::)/)"
_VALID: typ.Final[str] = f"filter = '{_SELECTOR}'\nsuccess-output = 'immediate'\n"
_UNRELATED: typ.Final[str] = "filter = 'test(/^some_other_test($|::)/)'\n"

EXPECTED_OFFENCE: typ.Final[dict[str, str]] = {
    "deleted": "found 0",
    "duplicated": "found 2",
    "filter-changed": "found 0",
    "filter-widened": "found 0",
    "mode-final": "success-output must be 'immediate'; found 'final'",
    "mode-absent": "success-output must be 'immediate'; found None",
    "group-added": "must not set test-group",
    "timeout-added": "timeout fields: ['slow-timeout']",
    "leak-timeout-added": "timeout fields: ['leak-timeout']",
    "in-another-profile": "found 0",
}
MUTATIONS: typ.Final[tuple[str, ...]] = ("valid", *EXPECTED_OFFENCE)


def mutate_success_output(mutation: str) -> str:
    """Return a valid TOML document with one named override mutation.

    Returns
    -------
    str
        A configuration with the named mutation and an unrelated override.

    Raises
    ------
    ValueError
        If the mutation name is unknown, so a typo cannot select an unmutated
        fixture.
    """
    bodies = {
        "valid": [_VALID],
        "deleted": [],
        "duplicated": [_VALID, _VALID],
        "filter-changed": [_VALID.replace(GUARDED_TEST, "some_other_test")],
        "filter-widened": [
            _VALID.replace(_SELECTOR, f"{_SELECTOR} | test(/^some_other_test($|::)/)")
        ],
        "mode-final": [_VALID.replace("'immediate'", "'final'")],
        "mode-absent": [_VALID.replace("success-output = 'immediate'\n", "")],
        "group-added": [_VALID + "test-group = 'nested-cargo-builds'\n"],
        "timeout-added": [
            _VALID + "slow-timeout = { period = '60s', terminate-after = 10 }\n"
        ],
        "leak-timeout-added": [_VALID + "leak-timeout = '100ms'\n"],
        "in-another-profile": [],
    }
    if mutation not in bodies:
        message = f"unknown success-output mutation: {mutation}"
        raise ValueError(message)
    entries = [_UNRELATED, *bodies[mutation]]
    document = "\n".join(f"[[profile.default.overrides]]\n{body}" for body in entries)
    if mutation == "in-another-profile":
        document += f"\n[[profile.ci.overrides]]\n{_VALID}"
    return document
