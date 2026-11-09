"""Hold the file and process effects the finalizer performs.

Every effect a finalization run needs lives here: the one allowlisted ``gh``
command, the diagnosis-and-exit helper, and the staged JSON records. Keeping
them together means the operations module that decides *when* to act imports
one module for *how* to act, and the exit statuses that vocabulary uses are
defined once.

Two properties are load-bearing. Every record is staged beside its destination
and moved into place only after it is written completely, so a failed run
leaves an earlier record untouched. And the ``gh`` call is launched through
Cuprum's catalogue, which passes each argument as one element and never
through a shell, so a tag cannot become a command.

A call that hangs is bounded by the composite action's ``timeout-minutes``:
the pinned Cuprum release has no in-process timeout.

Run via ``scripts/finalize_github_release.py``; behavioural coverage lives in
``scripts/tests/test_finalize_github_release_publish.py``.
"""

import json
import pathlib
import sys
import tempfile
import typing as typ

from cuprum import CommandResult, Program, ProgramCatalogue, sh
from cuprum.catalogue import ProjectSettings

GITHUB_CATALOGUE = ProgramCatalogue(
    projects=(
        ProjectSettings(
            name="finalize-github-release",
            programs=(Program("gh"),),
            documentation_locations=(),
            noise_rules=(),
        ),
    )
)

EXIT_OK = 0
EXIT_REFUSED = 1
EXIT_UPLOAD_REQUIRED = 2
EXIT_INPUT_UNREADABLE = 3
EXIT_OUTPUT_UNWRITABLE = 4
EXIT_REMOTE_FAILED = 5

#: Fields the snapshot needs. Naming them keeps ``gh`` from returning the
#: asset body or author metadata this stage would only discard.
VIEW_FIELDS = "tagName,isDraft,isPrerelease,assets"


def fail(code: int, message: str) -> typ.NoReturn:
    """Write one diagnosis to standard error and stop with ``code``.

    Raises
    ------
    SystemExit
        Always, carrying ``code``, so the workflow step fails with the status
        that names what went wrong rather than a traceback.

    Examples
    --------
    >>> import contextlib, io
    >>> hidden = io.StringIO()
    >>> try:
    ...     with contextlib.redirect_stderr(hidden):
    ...         fail(1, "the release remains a draft")
    ... except SystemExit as error:
    ...     print(error.code)
    1
    """
    print(message, file=sys.stderr)
    raise SystemExit(code)


def run_gh(*arguments: str, echo: bool = False) -> CommandResult:
    """Run one allowlisted GitHub CLI command.

    Returns
    -------
    CommandResult
        The captured output and exit status from ``gh``.

    A failed launch stops the run through :func:`fail` with
    :data:`EXIT_REMOTE_FAILED`, because no answer was produced to act on.

    Examples
    --------
    ``run_gh("release", "view", "v1.2.3", ...)`` passes the tag as one
    argument, never through a shell.
    """
    command = sh.make(Program("gh"), catalogue=GITHUB_CATALOGUE)(*arguments)
    try:
        return command.run_sync(echo=echo)
    except OSError as error:
        fail(
            EXIT_REMOTE_FAILED,
            f"gh could not be launched: {error}; the release remains a draft",
        )


def read_json(path: pathlib.Path) -> object:
    """Decode one JSON input, stopping when it cannot be read.

    Returns
    -------
    object
        The decoded value, unvalidated.

    Examples
    --------
    >>> with tempfile.TemporaryDirectory() as directory:
    ...     source = pathlib.Path(directory) / "record.json"
    ...     _ = source.write_text('{"a": 1}', encoding="utf-8")
    ...     read_json(source)
    {'a': 1}
    """
    try:
        return json.loads(path.read_bytes())
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        fail(EXIT_INPUT_UNREADABLE, f"{path} could not be read as JSON: {error}")


def write_json(path: pathlib.Path, value: object, *, context: str = "") -> None:
    """Stage a deterministic JSON record and move it into place.

    The temporary path is a sibling of ``path``, so the replacement stays on
    one filesystem and a reader never observes a partially written record.

    ``context`` is appended to the failure diagnosis. A record written after
    an irreversible step needs it: an operator reading "could not be written"
    alone cannot tell whether the release was left a draft.
    """
    text = json.dumps(value, indent=2, sort_keys=True) + "\n"
    try:
        with tempfile.TemporaryDirectory(
            dir=path.parent, prefix=f".{path.name}."
        ) as directory:
            staged = pathlib.Path(directory) / path.name
            _ = staged.write_text(text, encoding="utf-8")
            _ = staged.replace(path)
    except OSError as error:
        suffix = f"; {context}" if context else ""
        fail(EXIT_OUTPUT_UNWRITABLE, f"{path} could not be written: {error}{suffix}")


def write_github_output(
    path: pathlib.Path | None, outcome: str, *, is_prerelease: bool
) -> None:
    """Publish the step's two outputs when the workflow asked for them."""
    if path is None:
        return
    flag = "true" if is_prerelease else "false"
    try:
        with path.open("a", encoding="utf-8") as stream:
            _ = stream.write(f"outcome={outcome}\nprerelease={flag}\n")
    except OSError as error:
        fail(
            EXIT_OUTPUT_UNWRITABLE,
            f"{path} could not be written: {error}; the release itself is "
            "already final, but its outcome was not recorded",
        )
