"""What a `uses` reference must name, and how it must be pinned.

Several suites assert against external actions: the uv provisioning step, the
sccache installer, the artefact upload, and the shared coverage, packaging, and
release actions. Each of them wants the same two things — that the step calls
the action the repository depends on, and that it calls it at a full commit SHA
rather than a tag or branch that can move under it.

What none of them may assert is *which* revision that is. The correct SHA is
whatever the repository last pinned, so a test that spells it out says only
what Dependabot last wrote and fails on every bump that changed nothing. The
identity and the pin's shape are checked here instead, and the caller compares
the returned SHA against the other callers of the same action where the pins
have to agree.

This is why the helper returns the SHA rather than merely checking it: the
action a step names is a fact about this repository, while the revision it
names is not.

Run via ``make test-workflow-contracts``.
"""

import re

#: A full commit SHA pin: exactly 40 lowercase hexadecimal characters.
FULL_COMMIT_SHA_PATTERN = re.compile(r"^[0-9a-f]{40}$")


def require_external_action_sha(
    uses: object, expected_action: str, description: str
) -> str:
    """Return the commit SHA a step pins an external action to.

    A contract that names the action it depends on says something durable;
    one that repeats the revision says only what Dependabot last wrote. So
    the identity and the pin *shape* are asserted here, and callers compare
    the returned SHA against the other callers of the same action when the
    pins must agree. No expected SHA is taken, because the correct one is
    whatever the repository pinned, not a value a test can know.

    An assertion fails, naming ``description``, when ``uses`` is missing or
    empty, when it does not name ``expected_action``, or when what follows
    the final ``@`` is not a full 40-character lowercase SHA — a bare path
    with no separator, a tag, a branch, an abbreviated SHA, and a SHA in the
    wrong case are each rejected, and each reports which of those it saw.

    Parameters
    ----------
    uses
        The step's ``uses`` value, which may be missing or not a string.
    expected_action
        The action path the reference must name exactly, such as
        ``actions/checkout`` or ``owner/repo/.github/actions/name``. A
        leading ``./`` marks a local action, whose revision is the commit
        that holds it rather than a SHA in the reference.
    description
        What is being checked, for the failure message.

    Returns
    -------
    str
        The 40-character lowercase hexadecimal SHA after the final ``@``.
    """
    reference = str(uses or "")
    assert reference, f"{description} must use an action, got {uses!r}"
    assert not expected_action.startswith("./"), (
        f"{description} names the local action {expected_action!r}; a local "
        f"action is resolved from the checked-out commit and carries no pin"
    )
    assert "@" in reference, (
        f"{description} must pin {expected_action!r} to a commit SHA, got "
        f"{reference!r} with no '@' separator"
    )
    path, _, pin = reference.rpartition("@")
    assert path == expected_action, (
        f"{description} must use {expected_action!r}, got {path!r} in {reference!r}"
    )
    assert FULL_COMMIT_SHA_PATTERN.match(pin), (
        f"{description} must pin {expected_action!r} to a full 40-character "
        f"lowercase hexadecimal commit SHA, got {pin!r} in {reference!r}"
    )
    return pin
