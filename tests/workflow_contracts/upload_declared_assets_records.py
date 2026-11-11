"""Decode the records one executed action run leaves behind.

The action under test writes a ``GITHUB_OUTPUT`` file and names an interpreter
for each step it runs. Both are text formats of GitHub's, not of this
repository, so their decoders live here rather than in the harness that
happens to call them: a change to how a step is executed should not have to
touch the code that reads what the step wrote.

The heredoc form is the part worth naming. A multi-line diagnostic is written
as ``name<<DELIMITER``, the value's lines, then the delimiter again, and a
reader that split on ``=`` alone would silently record the delimiter as the
value. The entries this action writes never carry ``<<`` in a plain value,
which is what lets the two forms be told apart by the marker alone.

Run via ``make test-workflow-contracts``.
"""

import re
import typing as typ

from workflow_loading import REPO_ROOT

if typ.TYPE_CHECKING:
    import pathlib

__all__ = [
    "BASELINE_PATTERN",
    "interpreter_baseline",
    "read_outputs",
]

#: The Makefile declaration every step must agree with when it names an
#: interpreter, read rather than restated so a pin bump changes one file.
BASELINE_PATTERN = r"^PYTHON_BASELINE \?= (\S+)$"


def _take_heredoc(
    lines: list[str], start: int, header: str, outputs: dict[str, str]
) -> int:
    """Decode the heredoc output whose value lines begin at ``start``.

    Returns
    -------
    int
        The index just past the closing delimiter.
    """
    name, _marker, delimiter = header.partition("<<")
    end = start
    while end < len(lines) and lines[end] != delimiter:
        end += 1
    outputs[name] = "\n".join(lines[start:end])
    return end + 1


def read_outputs(path: pathlib.Path) -> dict[str, str]:
    """Decode a ``GITHUB_OUTPUT`` file, including its heredoc values.

    A heredoc value is written as ``name<<DELIMITER``, one or more lines, then
    the delimiter again, so a multi-line diagnostic survives the encoding. The
    entries this action writes never carry ``<<`` in a plain value, which is
    what lets the two forms be told apart by the marker alone.

    Returns
    -------
    dict[str, str]
        Output name mapped to its decoded value.
    """
    outputs: dict[str, str] = {}
    lines = path.read_text(encoding="utf-8").splitlines()
    index = 0
    while index < len(lines):
        line = lines[index]
        index += 1
        if "<<" in line:
            index = _take_heredoc(lines, index, line, outputs)
            continue
        name, _separator, plain = line.partition("=")
        outputs[name] = plain
    return outputs


def interpreter_baseline() -> str:
    """Return the interpreter the Makefile pins, which every step must name.

    Returns
    -------
    str
        The value of ``PYTHON_BASELINE`` in the Makefile.
    """
    text = (REPO_ROOT / "Makefile").read_text(encoding="utf-8")
    match = re.search(BASELINE_PATTERN, text, flags=re.MULTILINE)
    assert match is not None, "the Makefile must declare PYTHON_BASELINE ?="
    return match.group(1)
