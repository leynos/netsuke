"""Detect a `cargo install` that would compile a tool from source.

The estate installs CI tools from trusted prebuilt releases. One documented
exception remains, and one has been retired; both are policed with this
detector, so its blind spots are the policy's blind spots.

Its own tests live in `source_build_detector_test.py`. This module holds no
tests of its own.

Run via ``make test-workflow-contracts``.
"""

import re

#: The `cargo install` invocation, and everything up to the end of its line.
#: Continuations are joined before matching, so a command split across lines
#: is one candidate rather than several fragments.
_CARGO_INSTALL = re.compile(r"cargo\s+install\s+(?P<rest>.*)")


def _installed_crates(command: str) -> list[str]:
    """Return the crate arguments of one `cargo install` command.

    Parameters
    ----------
    command
        Everything following ``cargo install`` on the joined line.

    Returns
    -------
    list[str]
        Each non-option argument, stripped of quotes and of any ``@version``
        suffix.

    Notes
    -----
    Every non-option token is returned rather than only the first, because an
    option's value is indistinguishable from a crate name without knowing
    which options take one. `cargo install --version 0.9.1 cargo-orthohelp`
    therefore yields both ``0.9.1`` and ``cargo-orthohelp``. That is
    deliberate: callers compare against an exact crate name, so a stray
    version string matches nothing, whereas skipping tokens by shape let the
    crate hide behind an option value.
    """
    crates: list[str] = []
    for raw in command.split():
        token = raw.strip("\"'")
        if not token or token.startswith("-"):
            continue
        crates.append(token.split("@", 1)[0])
    return crates


def is_source_built(tool: str, workflow_text: str) -> bool:
    """Return whether any workflow compiles ``tool`` with `cargo install`.

    `cargo-orthohelp` was permitted a guarded source build while
    leynos/ortho-config#479 left the crate publishing no binaries. 0.9.1 ships
    archives for every platform this estate targets, so any `cargo install`
    naming it is now a regression.

    Parameters
    ----------
    tool
        The crate name, for example ``"cargo-orthohelp"``.
    workflow_text
        Every workflow and composite action concatenated.

    Returns
    -------
    bool
        ``True`` when some workflow installs ``tool`` from source.

    Notes
    -----
    The crate argument is compared exactly rather than by prefix, so
    ``cargo-orthohelp-extra`` is a different crate and does not match, and it
    is found among all of the command's non-option arguments, so an option
    value standing between the subcommand and the crate cannot conceal it.
    """
    joined = workflow_text.replace("\\\n", " ")
    return any(
        tool in _installed_crates(match["rest"])
        for match in _CARGO_INSTALL.finditer(joined)
    )
