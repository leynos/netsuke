"""Resolve the release-admission gate's configuration from the environment.

This module owns everything the gate reads *before* it runs an operation: the
required variables and diagnostics, the optional values and their defaults, the
artefact paths, and the adapter programs and sinks the run is wired from.

It is separate from :mod:`gate` because the two halves answer different
questions: this one asks what the gate was *told*, ``gate`` what it then
*does*. Keeping them apart keeps each reviewable alone: the default forms
below are Bash's, and getting one wrong changes admission behaviour silently.

Start-up is four steps rather than one, called in the order the shell refused
in. :func:`resolve_destination` and :func:`resolve_configuration` are pure;
:func:`prepare_destination` is the only step that touches a filesystem, through
the :class:`~.filesystem.FileSystem` it is handed; and
:func:`require_workflow_output` refuses the last required variable, *after* the
artefacts exist. That order is observable: the shell resolved its required
variables with ``:?``, ran one ``mkdir -p`` naming both parents, truncated both
files with ``: >``, and only then expanded ``${GITHUB_OUTPUT:?}``.

Both of Bash's default forms appear in the shell this gate replaces, and
:mod:`environment` owns them: the colon form for every path, sink, adapter, and
mode value, and the no-colon form for ``NETSUKE_RELEASE_ADMISSION_ENFORCE``
alone, so an exported empty mode is refused by the gate's own check.
"""

import dataclasses
import functools
import typing as typ
from pathlib import Path

from . import commands, environment, evidence, filesystem, policy, telemetry

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: Environment variables the gate requires, and the diagnostic each produces.
REPOSITORY_VARIABLE = "GITHUB_REPOSITORY"
REPOSITORY_DIAGNOSTIC = "GITHUB_REPOSITORY must identify the release repository"
REVISION_VARIABLE = "GITHUB_SHA"
REVISION_DIAGNOSTIC = "GITHUB_SHA must identify the release candidate revision"
RUN_ID_VARIABLE = "GITHUB_RUN_ID"
RUN_ID_DIAGNOSTIC = "GITHUB_RUN_ID must identify the workflow run being admitted"
OUTPUT_VARIABLE = "GITHUB_OUTPUT"
OUTPUT_DIAGNOSTIC = "GITHUB_OUTPUT must identify the workflow output file"

#: Environment variables the gate reads with a documented default.
TIMEOUT_VARIABLE = "NETSUKE_RELEASE_ADMISSION_OPERATION_TIMEOUT_SECONDS"
ENFORCE_VARIABLE = "NETSUKE_RELEASE_ADMISSION_ENFORCE"
METRICS_FILE_VARIABLE = "NETSUKE_RELEASE_ADMISSION_METRICS_FILE"
TRACE_FILE_VARIABLE = "NETSUKE_RELEASE_ADMISSION_TRACE_FILE"
TEMPORARY_DIRECTORY_VARIABLE = "RUNNER_TEMP"

#: The default artefact paths, relative to ``RUNNER_TEMP`` or the fallback.
METRICS_FILE_NAME = "netsuke-release-admission-metrics.jsonl"
TRACE_FILE_NAME = "netsuke-release-admission-traces.jsonl"
# The shell used ``${RUNNER_TEMP:-/tmp}``, so the fallback directory is part of
# the gate's contract rather than a temporary file this module invents.
DEFAULT_TEMPORARY_DIRECTORY = "/tmp"  # ruff: ignore[hardcoded-temp-file] - the documented fallback, never a temporary file.


@dataclasses.dataclass(frozen=True, slots=True)
class Configuration:
    """Hold the gate's resolved settings, with the shell's own defaults.

    ``output`` arrives later; ``tag`` and ``evidence_file`` are empty only in a
    rehearsal, which does not admit a release and therefore names neither.
    """

    repository: str
    revision: str
    run_id: str
    tag: str
    evidence_file: str
    output: str
    max_evidence_age_seconds: str
    timeout_seconds: str
    enforcement: str
    #: Whether this run can attribute the evidence to a producer it trusts.
    producer_trusted: bool
    programs: commands.Programs
    sinks: telemetry.Sinks


class ConfigurationError(Exception):
    """Report a required variable that is unset or empty, or a refused path.

    The message is the diagnostic itself, so the caller prints exactly the
    sentence the shell's ``:?`` form produced and nothing derived from the
    environment. The shell printed no sentence of its own for a filesystem
    refusal -- it let ``mkdir`` or the redirection report one -- so this class
    is also what carries the one bounded diagnostic the port prints in that
    state instead of escaping as an uncaught traceback.

    :func:`resolve_configuration` raises it directly and through
    :func:`_required`, as does :func:`require_workflow_output`, and
    :func:`prepare_destination` through :func:`_refusing`.
    """


def resolve_destination(variables: cabc.Mapping[str, str]) -> Destination:
    """Resolve both artefact paths, creating nothing.

    The mapping is a parameter rather than an ambient lookup, so a test can
    observe what resolution reads without a child process.

    Returns
    -------
    Destination
        Both configured spellings and the parent each one implies.
    """
    temporary = environment.optional(
        variables, TEMPORARY_DIRECTORY_VARIABLE, DEFAULT_TEMPORARY_DIRECTORY
    )
    return Destination(
        metrics_file=OutputPath.of(
            variables, METRICS_FILE_VARIABLE, f"{temporary}/{METRICS_FILE_NAME}"
        ),
        trace_file=OutputPath.of(
            variables, TRACE_FILE_VARIABLE, f"{temporary}/{TRACE_FILE_NAME}"
        ),
    )


def resolve_configuration(
    variables: cabc.Mapping[str, str], destination: Destination
) -> Configuration:
    """Resolve the gate's settings, performing no filesystem access.

    An unset or empty ``GITHUB_REPOSITORY``, ``GITHUB_SHA`` or ``GITHUB_RUN_ID``
    is refused through :func:`_required` with the shell's own diagnostic. An
    *enforcing* run must also name a tag and an evidence file; a rehearsal may
    leave both unset, which keeps the read-only lane reachable with no override.

    ``producer_trusted`` is true only for the exact text ``true``; every other
    state resolves to false, so the gate refuses evidence it cannot attribute.

    Returns
    -------
    Configuration
        The resolved variables, with both artefact spellings inside
        :attr:`Configuration.sinks`. :attr:`Configuration.output` is empty:
        see :func:`require_workflow_output` for why it arrives later.

    Raises
    ------
    ConfigurationError
        If a required variable is absent or empty, or if an enforcing run
        leaves the tag or the evidence file unset.

    Notes
    -----
    The artefact spellings reach the sinks here and are never read from the
    environment again. *destination* is passed in so one step answers once.
    """
    # Required variables are resolved first, and the artefacts are created
    # after them, because that is the order the shell refused them in and the
    # contract tests pin each refusal's position.
    repository = _required(variables, REPOSITORY_VARIABLE, REPOSITORY_DIAGNOSTIC)
    revision = _required(variables, REVISION_VARIABLE, REVISION_DIAGNOSTIC)
    run_id = _required(variables, RUN_ID_VARIABLE, RUN_ID_DIAGNOSTIC)
    enforcement = environment.defaulted_only_when_unset(
        variables, ENFORCE_VARIABLE, policy.ADMISSION_OBSERVATION_MODE
    )
    tag = environment.optional(variables, evidence.TAG_VARIABLE, "")
    evidence_file = environment.optional(variables, evidence.EVIDENCE_FILE_VARIABLE, "")
    # Publication trusts an enforcing run, so it must name the tag it is
    # admitting and the document that admits it. A rehearsal names neither.
    refusing = enforcement == policy.ADMISSION_ENFORCEMENT_MODE
    if refusing and not tag:
        raise ConfigurationError(evidence.TAG_DIAGNOSTIC)
    if refusing and not evidence_file:
        raise ConfigurationError(evidence.EVIDENCE_FILE_DIAGNOSTIC)
    return Configuration(
        repository=repository,
        revision=revision,
        run_id=run_id,
        tag=tag,
        evidence_file=evidence_file,
        output="",
        max_evidence_age_seconds=environment.optional(
            variables,
            evidence.MAX_AGE_VARIABLE,
            str(evidence.DEFAULT_EVIDENCE_MAX_AGE_SECONDS),
        ),
        timeout_seconds=environment.optional(
            variables, TIMEOUT_VARIABLE, str(policy.DEFAULT_OPERATION_TIMEOUT_SECONDS)
        ),
        enforcement=enforcement,
        producer_trusted=environment.optional(
            variables, evidence.PRODUCER_TRUST_VARIABLE, "false"
        )
        == "true",
        programs=commands.Programs.from_environment(variables),
        sinks=telemetry.Sinks(
            metrics_file=destination.metrics_file.configured,
            trace_file=destination.trace_file.configured,
            metrics_sink=_adapter(variables, commands.METRICS_SINK_VARIABLE),
            output_sink=_adapter(variables, commands.OUTPUT_SINK_VARIABLE),
            trace_sink=_adapter(variables, commands.TRACE_SINK_VARIABLE),
        ),
    )


def _adapter(variables: cabc.Mapping[str, str], variable: str) -> str:
    """Return a sink adapter's configured name, empty when unset."""
    return environment.optional(variables, variable, "")


def prepare_destination(
    sinks: telemetry.Sinks, file_system: filesystem.FileSystem
) -> None:
    """Create both artefact parents, then truncate both files.

    The bindings are read from *sinks* because that is where the configured
    spellings already live: resolution reads the environment once, so
    initialization and the workflow outputs cannot disagree about which file a
    run wrote.

    *file_system* is required rather than defaulted: an initializer that looked
    one up could only be shown to work by a real refusal, and the choice of
    filesystem would sit inside the effect rather than in the caller that owns
    the outcome. Either effect the operating system refuses reaches the caller
    as a :class:`ConfigurationError` from :func:`_refusing`, whose message
    names the configured spelling and keeps the system's own reason.

    Notes
    -----
    The two passes are the shell's, and the order is observable: ``mkdir -p``
    named *both* parents before either file was truncated, so a refused trace
    parent left a previous run's metrics file intact.
    """
    for target in sinks.bindings():
        _refusing(
            "release-admission could not create the directory for",
            target,
            functools.partial(file_system.make_parent, Path(_directory_name(target))),
        )
    for target in sinks.bindings():
        _refusing(
            "release-admission could not create",
            target,
            functools.partial(file_system.truncate, target),
        )


def require_workflow_output(variables: cabc.Mapping[str, str]) -> str:
    """Return ``GITHUB_OUTPUT``, refusing an unset or empty value.

    An unset or empty variable is refused with the diagnostic itself, so the
    caller prints the shell's own sentence and nothing derived from the
    environment.

    Returns
    -------
    str
        The configured workflow-output file.

    Notes
    -----
    This is the last refusal the shell made, *after* both artefact files
    existed: a run that dies here already has both files present and empty.
    """
    return _required(variables, OUTPUT_VARIABLE, OUTPUT_DIAGNOSTIC)


@dataclasses.dataclass(frozen=True, slots=True)
class Destination:
    """Hold the two artefact files the gate writes and names in its outputs.

    The paths are derived together and consumed together -- by the sinks, the
    artefact creation, and the workflow outputs -- so they travel as one value.
    """

    metrics_file: OutputPath
    trace_file: OutputPath


@dataclasses.dataclass(frozen=True, slots=True)
class OutputPath:
    """Carry one artefact path in both forms its two consumers need.

    ``configured`` is the string the operator supplied, verbatim, and
    ``directory`` is the parent the shell's ``mkdir -p "$(dirname "$file")"``
    named for it. They are not interchangeable: the configured string reaches
    *every* observable boundary -- the argument a sink adapter is named with,
    the workflow outputs, and the open that truncates the file -- so a path's
    original spelling survives wherever the shell preserved it. ``directory``
    is the ``mkdir`` operand alone, and :func:`_directory_name` is where its
    difference from :attr:`~pathlib.Path.parent` is set out.

    Rendering ``Path`` back to text at any of the configured boundaries would
    rewrite the operator's spelling, and at the truncation open it would change
    which file is written: ``Path("metrics.jsonl/")`` drops the slash, so the
    resolved form creates the regular file where the shell's redirection
    refused one. A trailing slash is a filesystem distinction rather than a
    cosmetic one, which is why :class:`~.filesystem.FileSystem` truncates by
    the configured spelling and makes the parent by this operand.
    """

    configured: str
    directory: Path

    @classmethod
    def of(
        cls, variables: cabc.Mapping[str, str], variable: str, default: str
    ) -> OutputPath:
        """Resolve one artefact path from its variable and default."""
        configured = environment.optional(variables, variable, default)
        return cls(configured=configured, directory=Path(_directory_name(configured)))


def _required(variables: cabc.Mapping[str, str], variable: str, diagnostic: str) -> str:
    """Return a required variable, refusing an unset or empty value."""
    value = variables.get(variable)
    if not value:
        raise ConfigurationError(diagnostic)
    return value


def _directory_name(configured: str) -> str:
    """Return the directory the shell's ``dirname`` named for *configured*.

    The shell computed the ``mkdir`` operand as ``$(dirname "$file")``, so the
    operand is coreutils ``dirname(1)`` applied to the operator's own string --
    not the configured path's :attr:`~pathlib.Path.parent`, and not
    :func:`os.path.dirname` either. All three disagree somewhere, and the
    disagreement that matters is a final ``/.``: ``dirname(1)`` answers
    ``reports`` for ``reports/.``, ``Path("reports/.").parent`` answers ``.``,
    and making the latter is a no-op, so the truncation would then fail with
    ``No such file or directory`` where the shell had already created
    ``reports`` and failed with ``Is a directory``.

    :func:`os.path.dirname` is the near miss rather than the answer: it answers
    ``reports`` for ``reports/`` where ``dirname(1)`` answers ``.``, which would
    put a new divergence where the trailing-slash cases already look.

    Returns
    -------
    str
        The parent directory of the configured string, spelled as the shell's
        ``dirname`` spelled it.

    Examples
    --------
    >>> _directory_name("reports/."), _directory_name("reports/")
    ('reports', '.')
    >>> _directory_name("/obs//m.jsonl"), _directory_name("obs//")
    ('/obs', '.')
    >>> _directory_name("/"), _directory_name("")
    ('/', '.')
    """
    # dirname(1)'s own algorithm: strip the trailing separators first, so a
    # string that was *only* separators has no final component to shed, then
    # shed everything from the last remaining separator onwards. A path with no
    # separator left has no directory part and names the current directory; one
    # whose directory part is empty is the filesystem root.
    stripped = configured.rstrip("/")
    if not stripped:
        return "/" if configured.startswith("/") else "."
    parent, separator, _ = stripped.rpartition("/")
    if not separator:
        return "."
    return parent.rstrip("/") or "/"


def _refusing(lead: str, target: str, operation: cabc.Callable[[], None]) -> None:
    """Perform one filesystem operation on *target*, refusing a rejection.

    ``mkdir`` and the redirection each reported their own sentence, and both
    reach the caller as one bounded diagnostic, keeping the system's text
    through :func:`_reason` so the operator still reads *why* the path was
    refused.

    Raises
    ------
    ConfigurationError
        When the operating system refuses the operation.
    """
    try:
        operation()
    except OSError as error:
        msg = f"{lead} {target}: {_reason(error)}"
        raise ConfigurationError(msg) from error


def _reason(error: OSError) -> str:
    """Render one operating-system refusal as a short reason.

    The shell printed the operating system's own sentence, so this carries its
    text whenever the system supplied one, and the class name otherwise.

    Returns
    -------
    str
        The system's own description, or the error's class name without one.
    """
    return error.strerror or error.__class__.__name__
