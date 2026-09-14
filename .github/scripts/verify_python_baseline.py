"""Refuse to run trusted workflow steps on the wrong Python interpreter.

The trusted coverage workflow runs its steps through GitHub Actions' ``python``
shell, which resolves ``python`` on ``PATH``. The other trusted modules are
written to the repository's Python baseline: under an older interpreter they
fail as they load, with a ``NameError`` far from its cause, because their
annotations name imports made only under ``typing.TYPE_CHECKING``. This module
therefore uses only built-in names in its annotations, so it loads anywhere,
and fails by name when the interpreter is not the baseline.

``BASELINE`` is held equal to the Makefile's ``PYTHON_BASELINE`` by
``tests/workflow_contracts/python_shell_interpreter_test.py``.
"""

import argparse
import sys

#: The Python major and minor version every trusted workflow step runs under.
BASELINE = (3, 14)

COMMANDS = ("verify",)


def baseline_mismatch(version_info: tuple[int, ...], executable: str) -> str | None:
    """Describe how an interpreter differs from the baseline, if it does.

    Parameters
    ----------
    version_info
        The first three items of the interpreter's ``sys.version_info``: its
        major, minor, and micro versions.
    executable
        Path of the interpreter, reported so the wrong ``PATH`` entry is
        visible in the failure.

    Returns
    -------
    str | None
        ``None`` when the major and minor versions equal ``BASELINE``;
        otherwise a message naming the required and found versions.
    """
    if tuple(version_info[:2]) == BASELINE:
        return None
    required = ".".join(str(part) for part in BASELINE)
    found = ".".join(str(part) for part in version_info[:3])
    return (
        f"trusted workflow steps need Python {required}, found {found} at {executable}"
    )


def main(argv: list[str] | None = None) -> int:
    """Exit non-zero unless the running interpreter is the baseline.

    Parameters
    ----------
    argv
        Command-line arguments; defaults to ``sys.argv[1:]``.

    Returns
    -------
    int
        ``0`` when the interpreter matches ``BASELINE``, ``1`` otherwise.
    """
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=COMMANDS)
    parser.parse_args(argv)
    mismatch = baseline_mismatch(tuple(sys.version_info[:3]), sys.executable)
    if mismatch is None:
        print(f"python {sys.version.split()[0]} at {sys.executable}")
        return 0
    print(mismatch, file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
