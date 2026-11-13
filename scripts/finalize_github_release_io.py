"""Hold the file and process effects the finalizer performs.

Every effect a finalization run needs lives here: the one allowlisted ``gh``
command, the failure type that names an exit status, and the staged JSON
records. Keeping them together means the operations module that decides *when*
to act imports one module for *how* to act, and the exit statuses that
vocabulary uses are defined once.

Two properties are load-bearing. Every record is staged beside its destination
and moved into place only after it is written completely, so a failed run
leaves an earlier record untouched. And the ``gh`` call is launched through
Cuprum's catalogue, which passes each argument as one element and never
through a shell, so a tag cannot become a command.

A call that hangs is bounded by the GNU ``timeout`` each action step wraps
around its adapter invocation: the pinned Cuprum release has no in-process
timeout, and the runner does not support ``timeout-minutes`` inside a
composite action.

Every helper that cannot continue raises :class:`FinalizeError`, which
carries the exit status the workflow branches on and the diagnosis an operator
reads. Each helper therefore declares that failure under *Raises*, and the
command layer -- not the helper -- turns it into a process exit, so a function
that decodes a record or runs a command never decides the step's status.

Run via ``scripts/finalize_github_release.py``; behavioural coverage lives in
``scripts/tests/test_finalize_github_release_publish.py``.
"""

import json
import pathlib
import tempfile
import typing as typ

import release_digest_record
import release_digest_record_files
from cuprum import CommandResult, Program, ProgramCatalogue, sh
from cuprum.catalogue import ProjectSettings

if typ.TYPE_CHECKING:
    # The alias is evaluated lazily, so naming ``Callable`` here costs a
    # start-up import nothing.
    import collections.abc as cabc

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

#: The shape of the one process effect a run performs. Every adapter that runs
#: ``gh`` takes one of these, so a decision can be exercised with a plain
#: function that returns recorded answers and no process is started.
type Runner = cabc.Callable[..., CommandResult]


class FinalizeError(Exception):
    """Carry the exit status and diagnosis of a run that cannot continue.

    Parameters
    ----------
    code
        The exit status the workflow step reports for this failure.
    message
        The diagnosis an operator reads. It names what failed and, where it
        matters, what the run has established about the release.

    Notes
    -----
    A helper raises this rather than ending the process, so the status travels
    through the call stack as data and the command layer decides when to exit.
    That keeps a function's fallibility in its signature instead of in a
    process effect a reader has to trace.

    Examples
    --------
    >>> failure = FinalizeError(EXIT_REFUSED, "the plan still owes an upload")
    >>> failure.code, str(failure)
    (1, 'the plan still owes an upload')
    """

    def __init__(self, code: int, message: str) -> None:
        """Store the exit status alongside the diagnosis it explains."""
        super().__init__(message)
        self.code = code


def run_gh(*arguments: str, echo: bool = False) -> CommandResult:
    """Run one allowlisted GitHub CLI command.

    Parameters
    ----------
    *arguments
        The arguments to ``gh``, one element each, so neither a tag nor a
        repository name can become a second command or a flag.
    echo
        Whether Cuprum echoes the command before running it. The single
        mutating call sets it, so the log names what changed the release.

    Returns
    -------
    CommandResult
        The captured output and exit status from ``gh``.

    Raises
    ------
    FinalizeError
        With :data:`EXIT_REMOTE_FAILED` when ``gh`` could not be launched,
        because no answer was produced to act on. The diagnosis claims nothing
        about the release: a launch failure on the edit and one on a read
        leave the same evidence, which is none. A command that runs and exits
        non-zero is not a failure here: its status and stderr are the answer,
        and the caller decides what a given status means.

    Examples
    --------
    ``run_gh("release", "view", "v1.2.3", ...)`` passes the tag as one
    argument, never through a shell.
    """
    command = sh.make(Program("gh"), catalogue=GITHUB_CATALOGUE)(*arguments)
    try:
        return command.run_sync(echo=echo)
    except OSError as error:
        raise FinalizeError(
            EXIT_REMOTE_FAILED,
            f"gh could not be launched: {error}; the release state is unknown",
        ) from error


def read_json(path: pathlib.Path) -> object:
    """Decode one JSON input, stopping when it cannot be read.

    Parameters
    ----------
    path
        The record to decode, which this stage only ever reads.

    Returns
    -------
    object
        The decoded value, unvalidated: each reader checks the shape it needs,
        because "valid JSON" and "a usable record" are different claims.

    Raises
    ------
    FinalizeError
        With :data:`EXIT_INPUT_UNREADABLE` when the file is absent, is not
        text, or does not parse.

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
        raise FinalizeError(
            EXIT_INPUT_UNREADABLE, f"{path} could not be read as JSON: {error}"
        ) from error


def load_digest_record(path: pathlib.Path) -> release_digest_record.DigestRecord:
    """Decode the trusted digest record the declaration stage wrote.

    Reading the record is an adapter's job, so the decision layer receives a
    validated value and never a path. That is what lets a test ask what a run
    decides about a changed asset without writing a file.

    Parameters
    ----------
    path
        The record the declaration stage retained before upload.

    Returns
    -------
    release_digest_record.DigestRecord
        The validated record with lowercase digests.

    Raises
    ------
    FinalizeError
        With :data:`EXIT_INPUT_UNREADABLE` when the record is absent, is not
        JSON text, or does not describe a usable digest record. An absent
        record would reconcile nothing, and none of those shapes is a verified
        release.

    Examples
    --------
    >>> with tempfile.TemporaryDirectory() as directory:
    ...     source = pathlib.Path(directory) / "digests.json"
    ...     _ = source.write_text(
    ...         '{"assets": {"a.deb": "' + "ab" * 32 + '"}}', encoding="utf-8"
    ...     )
    ...     load_digest_record(source).as_mapping()["a.deb"] == "ab" * 32
    True
    """
    try:
        return release_digest_record_files.read_digest_record(path)
    except release_digest_record.DigestRecordError as error:
        raise FinalizeError(
            EXIT_INPUT_UNREADABLE, f"{path} is not a usable digest record: {error}"
        ) from error


def write_json(path: pathlib.Path, value: object, *, context: str = "") -> None:
    """Stage a deterministic JSON record and move it into place.

    The temporary path is a sibling of ``path``, so the replacement stays on
    one filesystem and a reader never observes a partially written record.

    Parameters
    ----------
    path
        The record's final location. Its parent must exist.
    value
        The value to encode, as indented and key-sorted JSON, so two runs that
        decide the same thing record byte-identical files.
    context
        A sentence appended to the failure diagnosis. A record written after an
        irreversible step needs one: an operator reading "could not be written"
        alone cannot tell whether the release was left a draft.

    Raises
    ------
    FinalizeError
        With :data:`EXIT_OUTPUT_UNWRITABLE` when the record cannot be staged
        or moved into place: a run whose decision cannot be recorded must not
        report success and leave the next attempt to guess.

    Examples
    --------
    >>> with tempfile.TemporaryDirectory() as directory:
    ...     target = pathlib.Path(directory) / "plan.json"
    ...     write_json(target, {"actions": []})
    ...     json.loads(target.read_text(encoding="utf-8"))
    {'actions': []}
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
        raise FinalizeError(
            EXIT_OUTPUT_UNWRITABLE, f"{path} could not be written: {error}{suffix}"
        ) from error


def write_github_output(
    path: pathlib.Path | None, outcome: str, *, is_prerelease: bool
) -> None:
    """Publish the step's two outputs when the workflow asked for them.

    Parameters
    ----------
    path
        The workflow's output file, or ``None`` when the step asked for no
        outputs, which is why a missing destination is not a failure here.
    outcome
        ``finalized`` or ``already-finalized``: which of the two ways this run
        left the release public.
    is_prerelease
        The validated candidate's prerelease state, written as ``true`` or
        ``false`` rather than Python's capitalized spelling.

    Raises
    ------
    FinalizeError
        With :data:`EXIT_OUTPUT_UNWRITABLE` when the file cannot be appended
        to. The release is already public at that point, so the diagnosis says
        so: the outcome is lost, not the publication.
    """
    if path is None:
        return
    flag = "true" if is_prerelease else "false"
    try:
        with path.open("a", encoding="utf-8") as stream:
            _ = stream.write(f"outcome={outcome}\nprerelease={flag}\n")
    except OSError as error:
        raise FinalizeError(
            EXIT_OUTPUT_UNWRITABLE,
            f"{path} could not be written: {error}; the release itself is "
            "already final, but its outcome was not recorded",
        ) from error
