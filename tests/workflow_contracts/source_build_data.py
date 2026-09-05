"""Detect a `cargo install` that would compile a tool from source.

The estate installs CI tools from trusted prebuilt releases. One documented
exception remains, and one has been retired; both are policed with this
detector, so its blind spots are the policy's blind spots.

Its own tests live in `source_build_detector_test.py`. This module holds no
tests of its own.

Run via ``make test-workflow-contracts``.
"""

import re

#: Everything on a line from `cargo` onwards. Which of those lines is actually
#: an install is decided by tokenizing, not by this pattern: cargo accepts a
#: `+toolchain` selector and any number of global options before its
#: subcommand, and an option may take a separate value, so no fixed pattern
#: recognises the set.
_CARGO_COMMAND = re.compile(r"cargo\s+(?P<rest>.*)")


def _is_option_value(tokens: list[str], index: int) -> bool:
    """Return whether the word at ``index`` is a preceding option's value.

    Parameters
    ----------
    tokens
        The command's tokens.
    index
        Position of the token following an option that carried no ``=``.

    Returns
    -------
    bool
        ``True`` when the token exists, is not another option, and is not the
        ``install`` subcommand. A flag takes no value, and reading ``install``
        as one would hide the command being looked for.
    """
    if index >= len(tokens):
        return False
    word = tokens[index]
    return word != "install" and not word.startswith(("-", "+"))


def _skip_prefix(tokens: list[str]) -> int:
    """Return the index of the subcommand, skipping selectors and options.

    Parameters
    ----------
    tokens
        The command's tokens, excluding the leading ``cargo``.

    Returns
    -------
    int
        Index of the first ordinary word, or ``len(tokens)`` when the command
        is nothing but a prefix.
    """
    index = 0
    while index < len(tokens) and tokens[index].startswith(("+", "-")):
        word = tokens[index]
        index += 1
        if not word.startswith("-") or "=" in word:
            continue
        if _is_option_value(tokens, index):
            index += 1
    return index


def _install_arguments(command: str) -> list[str] | None:
    """Return the arguments after `install`, or ``None`` if this is not one.

    Parameters
    ----------
    command
        Everything following the word ``cargo`` on the joined line.

    Returns
    -------
    list[str] or None
        The remaining tokens when the subcommand is ``install``, otherwise
        ``None``.

    Notes
    -----
    Tokens before the subcommand are skipped rather than matched. A
    ``+toolchain`` selector, a global flag, and a global option with a
    separate value such as ``--config net.retry=1`` may all precede
    ``install``, while an option carrying its value inline with ``=`` takes no
    following token. A pattern that tried to spell that set would be wrong in
    a way nobody would notice, which is how ``cargo  install`` got past its
    predecessor.

    An option spelled ``--name=value`` carries its own value; one spelled
    ``--name value`` consumes the word after it. Ambiguity resolves toward
    detection. Whether ``--flag install`` means a
    flag followed by the subcommand or an option whose value is ``install``
    cannot be settled without cargo's own option table, so the subcommand
    reading wins: this guards a prohibition, where a false positive is
    arguable and a false negative is a hole.
    """
    tokens = command.split()
    index = _skip_prefix(tokens)
    if index >= len(tokens) or tokens[index] != "install":
        return None
    return tokens[index + 1 :]


def joined_commands(workflow_text: str) -> str:
    """Return ``workflow_text`` with shell line continuations folded away.

    Parameters
    ----------
    workflow_text
        Every workflow and composite action concatenated.

    Returns
    -------
    str
        The same text with backslash line continuations joined, so a command
        split across lines is one candidate rather than several fragments.
    """
    return workflow_text.replace("\\\n", " ")


def source_build_commands(workflow_text: str) -> list[str]:
    """Return every `cargo install` command found, as written.

    Parameters
    ----------
    workflow_text
        Every workflow and composite action concatenated.

    Returns
    -------
    list[str]
        One entry per invocation, each being the arguments following the
        subcommand. Empty when nothing compiles a tool, which is the state
        this repository holds itself to.
    """
    found = []
    for match in _CARGO_COMMAND.finditer(joined_commands(workflow_text)):
        arguments = _install_arguments(match["rest"])
        if arguments is not None:
            found.append(" ".join(arguments))
    return found


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
    Every non-option word is returned rather than only the first, because an
    option's value is indistinguishable from a crate name without knowing
    which options take one. `cargo install --version 0.9.1 cargo-orthohelp`
    therefore yields both ``0.9.1`` and ``cargo-orthohelp``. That is
    deliberate: callers compare against an exact crate name, so a stray
    version string matches nothing, whereas skipping tokens by shape let the
    crate hide behind an option value.
    """
    crates: list[str] = []
    for raw in command.split():
        word = raw.strip("\"'")
        if not word or word.startswith("-"):
            continue
        crates.append(word.split("@", 1)[0])
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
    return any(
        tool in _installed_crates(command)
        for command in source_build_commands(workflow_text)
    )
