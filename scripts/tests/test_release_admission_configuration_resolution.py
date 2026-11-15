r"""Pin that configuration resolution is pure and reads the mapping it is given.

Resolution used to be inseparable from initialization: one ``load_configuration``
both read the variables and created both artefacts, so a reader could not tell
which of its consequences came from reading and which from writing. It is now
two steps with a seam between them, and this module pins the near side:
:func:`~_release_admission.configuration.resolve_destination` derives the two
artefact paths, :func:`~_release_admission.configuration.resolve_configuration`
reads every documented default into one
:class:`~_release_admission.configuration.Configuration`, and
:func:`~_release_admission.configuration.require_workflow_output` reads the one
variable the shell left until after both artefacts existed.

Where the artefacts go, and the spelling each is named by, is
:mod:`test_release_admission_destination_resolution`'s subject; the cases here
start from a destination and vary only the settings.

Every case builds its own mapping in this process: resolution cannot consult
``os.environ``, so the ambient environment cannot make a case pass, and the
in-process boundary is what lets the absence of a filesystem effect be
asserted. The effects themselves, and their order, belong to
:mod:`test_release_admission_artefact_initialization`, which is given the port
directly for the same reason.

Example (run from the repository root)::

    PYTHONPATH=scripts uv run --no-project --python 3.14 \
        --with pytest==9.0.2 --with 'cuprum==0.1.0' \
        python -m pytest \
        scripts/tests/test_release_admission_configuration_resolution.py \
        -c /dev/null --rootdir=. -p no:cacheprovider
"""

import sys
import typing as typ
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / ".github" / "scripts"))
from _release_admission import commands, configuration, evidence, policy, telemetry

if typ.TYPE_CHECKING:
    import collections.abc as cabc

REPOSITORY_VARIABLE = "GITHUB_REPOSITORY"
REVISION_VARIABLE = "GITHUB_SHA"
OUTPUT_VARIABLE = "GITHUB_OUTPUT"
RUN_ID_VARIABLE = "GITHUB_RUN_ID"
TAG_VARIABLE = "NETSUKE_RELEASE_ADMISSION_TAG"
EVIDENCE_FILE_VARIABLE = "NETSUKE_RELEASE_ADMISSION_EVIDENCE_FILE"
MAX_AGE_VARIABLE = "NETSUKE_RELEASE_ADMISSION_MAX_EVIDENCE_AGE_SECONDS"
ENFORCE_VARIABLE = "NETSUKE_RELEASE_ADMISSION_ENFORCE"
TIMEOUT_VARIABLE = "NETSUKE_RELEASE_ADMISSION_OPERATION_TIMEOUT_SECONDS"
METRICS_FILE_VARIABLE = "NETSUKE_RELEASE_ADMISSION_METRICS_FILE"
TRACE_FILE_VARIABLE = "NETSUKE_RELEASE_ADMISSION_TRACE_FILE"
TEMPORARY_DIRECTORY_VARIABLE = "RUNNER_TEMP"

REPOSITORY = "leynos/netsuke"
REVISION = "0123456789abcdef0123456789abcdef01234567"
RUN_ID = "1001"

#: The temporary directory every case gives the gate, so that no case can
#: observe the ambient ``RUNNER_TEMP`` or reach the ``/tmp`` fallback.
TEMPORARY_DIRECTORY = "/nonexistent/runner-temp"

#: The variables every case begins from, so each case states only its subject.
BASELINE_ENVIRONMENT = {
    REPOSITORY_VARIABLE: REPOSITORY,
    REVISION_VARIABLE: REVISION,
    RUN_ID_VARIABLE: RUN_ID,
    TEMPORARY_DIRECTORY_VARIABLE: TEMPORARY_DIRECTORY,
}


def resolve(
    variables: cabc.Mapping[str, str],
) -> tuple[configuration.Destination, configuration.Configuration]:
    """Resolve one mapping as the entry point's first two steps do.

    Returns
    -------
    tuple[configuration.Destination, configuration.Configuration]
        The artefact paths, then the settings resolved against them.
    """
    destination = configuration.resolve_destination(variables)
    return destination, configuration.resolve_configuration(variables, destination)


@pytest.mark.parametrize(
    ("variable", "diagnostic"),
    [
        pytest.param(REPOSITORY_VARIABLE, configuration.REPOSITORY_DIAGNOSTIC),
        pytest.param(REVISION_VARIABLE, configuration.REVISION_DIAGNOSTIC),
        pytest.param(RUN_ID_VARIABLE, configuration.RUN_ID_DIAGNOSTIC),
    ],
)
@pytest.mark.parametrize("exported", ["", None], ids=["empty", "unset"])
def test_missing_required_variable_is_refused_with_the_shell_diagnostic(
    variable: str, diagnostic: str, exported: str | None
) -> None:
    """Verify a required variable is refused with the shell's own sentence.

    Notes
    -----
    An unset value and an exported-empty one are both refused, because the
    shell expanded both through ``:?``. The message is the diagnostic itself,
    not a sentence this step composes, so the caller prints the shell's wording
    without holding a second copy of it.
    """
    destination = configuration.resolve_destination(BASELINE_ENVIRONMENT)
    variables = {
        key: value for key, value in BASELINE_ENVIRONMENT.items() if key != variable
    }
    if exported is not None:
        variables[variable] = exported

    with pytest.raises(configuration.ConfigurationError) as refusal:
        configuration.resolve_configuration(variables, destination)

    assert str(refusal.value) == diagnostic, (
        "the refusal must carry the shell's own sentence rather than a new one"
    )


def test_documented_defaults_are_read_into_one_configuration() -> None:
    """Verify the settings hold each default the shell documented.

    Notes
    -----
    ``GITHUB_OUTPUT`` is the one value resolution leaves empty, and the case
    pins that rather than leaving it to the comment in
    :class:`~_release_admission.configuration.Configuration`: it is what keeps
    the refusal until after both artefacts exist.
    """
    _, settings = resolve(BASELINE_ENVIRONMENT)

    assert settings.repository == REPOSITORY, (
        "GITHUB_REPOSITORY must reach the settings unchanged"
    )
    assert settings.revision == REVISION, "GITHUB_SHA must reach the settings unchanged"
    assert not settings.output, (
        "GITHUB_OUTPUT must stay unresolved until the artefacts exist"
    )
    assert settings.run_id == RUN_ID, "GITHUB_RUN_ID must reach the settings unchanged"
    assert not settings.tag, (
        "an unset tag must stay empty, which is what makes a rehearsal reachable"
    )
    assert not settings.evidence_file, (
        "an unset evidence file must stay empty, as a rehearsal names no document"
    )
    assert settings.max_evidence_age_seconds == str(
        evidence.DEFAULT_EVIDENCE_MAX_AGE_SECONDS
    ), "an unset maximum age must resolve to the gate's own default, as a string"
    assert settings.timeout_seconds == str(policy.DEFAULT_OPERATION_TIMEOUT_SECONDS), (
        "an unset timeout must resolve to the gate's own default, as a string"
    )
    assert settings.enforcement == policy.ADMISSION_OBSERVATION_MODE, (
        "an unset enforcement mode must resolve to the documented observation mode"
    )


def test_a_configured_value_is_read_in_place_of_every_default() -> None:
    """Verify a configured value is read wherever the shell read one.

    Notes
    -----
    Each variable is given a value no default could produce, so a default
    leaking into a field fails the case rather than coinciding with it.
    """
    _, settings = resolve({
        **BASELINE_ENVIRONMENT,
        RUN_ID_VARIABLE: "4242",
        TAG_VARIABLE: "v0.10.0",
        EVIDENCE_FILE_VARIABLE: "/configured/evidence.json",
        MAX_AGE_VARIABLE: "3600",
        TIMEOUT_VARIABLE: "17",
        ENFORCE_VARIABLE: "true",
        METRICS_FILE_VARIABLE: "/configured/metrics.jsonl",
        TRACE_FILE_VARIABLE: "/configured/traces.jsonl",
    })

    assert settings.run_id == "4242", (
        "a configured run identifier must be read rather than defaulted"
    )
    assert settings.tag == "v0.10.0", (
        "a configured tag must be read rather than defaulted"
    )
    assert settings.evidence_file == "/configured/evidence.json", (
        "a configured evidence file must be read rather than defaulted"
    )
    assert settings.max_evidence_age_seconds == "3600", (
        "a configured maximum age must be read rather than defaulted"
    )
    assert settings.timeout_seconds == "17", (
        "a configured timeout must be read rather than defaulted"
    )
    assert settings.enforcement == "true", (
        "a configured enforcement mode must be read rather than defaulted"
    )
    assert settings.sinks.metrics_file == "/configured/metrics.jsonl", (
        "a configured metrics path must be read rather than defaulted"
    )
    assert settings.sinks.trace_file == "/configured/traces.jsonl", (
        "a configured trace path must be read rather than defaulted"
    )


def test_enforcement_keeps_an_exported_empty_value_where_a_path_defaults_it() -> None:
    """Verify the two Bash default forms stay distinguishable through resolution.

    Notes
    -----
    This is the whole reason the forms have two names. An empty artefact path
    becomes the default path, while an empty enforcement mode survives into the
    settings and is refused by the gate's own mode check -- which stops a
    configuration the gate rejects from running as observation. One helper for
    both cannot satisfy both halves at once.
    """
    _, settings = resolve({
        **BASELINE_ENVIRONMENT,
        METRICS_FILE_VARIABLE: "",
        ENFORCE_VARIABLE: "",
    })

    assert not settings.enforcement, (
        "the no-colon form must keep an exported empty enforcement mode"
    )
    assert settings.sinks.metrics_file == (
        f"{TEMPORARY_DIRECTORY}/{configuration.METRICS_FILE_NAME}"
    ), "the colon form must substitute the default for an empty path"


@pytest.mark.parametrize(
    ("variable", "diagnostic"),
    [
        pytest.param(TAG_VARIABLE, evidence.TAG_DIAGNOSTIC, id="tag"),
        pytest.param(
            EVIDENCE_FILE_VARIABLE, evidence.EVIDENCE_FILE_DIAGNOSTIC, id="evidence"
        ),
    ],
)
def test_enforcing_without_its_inputs_is_refused_before_anything_runs(
    variable: str, diagnostic: str
) -> None:
    """Verify an enforcing run must name the tag and the document it admits.

    Notes
    -----
    Publication trusts an enforcing run's success, so an enforcing run that
    named no tag would admit whatever the manifest said about a tag nobody
    published, and one that named no document would have nothing to verify.
    Both refusals happen in resolution, before any artefact is created, so a
    misconfigured publishing run leaves no records to misread as an admission.
    """
    destination = configuration.resolve_destination(BASELINE_ENVIRONMENT)
    variables = {
        **BASELINE_ENVIRONMENT,
        ENFORCE_VARIABLE: policy.ADMISSION_ENFORCEMENT_MODE,
        TAG_VARIABLE: "v0.10.0",
        EVIDENCE_FILE_VARIABLE: "/configured/evidence.json",
    }
    del variables[variable]

    with pytest.raises(configuration.ConfigurationError) as refusal:
        configuration.resolve_configuration(variables, destination)

    assert str(refusal.value) == diagnostic, (
        "the refusal must name the variable the enforcing run failed to supply"
    )


def test_adapter_names_and_sinks_are_resolved_for_the_rest_of_the_run() -> None:
    """Verify resolution wires the adapter names and the three sinks.

    Notes
    -----
    The artefact bindings are asserted against the *destination*, not a second
    read of the environment, because that identity is the contract: the spelling
    the sinks carry is the one the workflow outputs publish and the one
    initialization truncates, with no third answer to disagree with.
    """
    destination, settings = resolve(BASELINE_ENVIRONMENT)

    assert settings.programs.names() == (
        commands.DEFAULT_GH_ADAPTER,
        commands.DEFAULT_GIT_ADAPTER,
        commands.DEFAULT_CLOCK_ADAPTER,
    ), "each unset adapter must resolve to its documented program name"
    assert isinstance(settings.sinks, telemetry.Sinks), (
        "resolution must build the sinks value rather than some other carrier"
    )
    assert settings.sinks.bindings() == (
        destination.metrics_file.configured,
        destination.trace_file.configured,
    ), "the sinks must carry the resolved spellings, in the shell's order"
    assert (
        settings.sinks.metrics_sink,
        settings.sinks.output_sink,
        settings.sinks.trace_sink,
    ) == ("", "", ""), "an unset sink adapter must select the in-process append"


def test_resolution_writes_nothing_to_the_filesystem(tmp_path: Path) -> None:
    """Verify resolution performs no filesystem access at all.

    Notes
    -----
    Both targets sit below a directory that must not come into existence, so a
    resolver that created an artefact fails by having created one. The directory
    the paths resolve against is then checked empty, which is the half a single
    target's absence cannot state.
    """
    spelled = tmp_path / "unwritten"
    _, settings = resolve({
        **BASELINE_ENVIRONMENT,
        METRICS_FILE_VARIABLE: str(spelled / "metrics.jsonl"),
        TRACE_FILE_VARIABLE: str(spelled / "traces.jsonl"),
    })

    assert settings.sinks.metrics_file == str(spelled / "metrics.jsonl"), (
        "the settings must carry the configured path it was resolved from"
    )
    assert not spelled.exists(), (
        "resolution must derive the paths without creating either of them"
    )
    assert not list(tmp_path.iterdir()), (
        "resolution must leave the directory it resolved against untouched"
    )


def test_resolving_the_same_mapping_twice_gives_one_answer() -> None:
    """Verify resolution is a function of the mapping it is handed.

    Notes
    -----
    Purity is asserted as an equality of two results rather than as the absence
    of a side effect, because a resolver that read the ambient environment would
    answer differently only when the ambient value differed. An equality fails
    when resolution caches something the mapping does not carry, or when one of
    the two results carries state the other does not.
    """
    first_destination, first = resolve(BASELINE_ENVIRONMENT)
    second_destination, second = resolve(BASELINE_ENVIRONMENT)

    assert first_destination == second_destination, (
        "destination resolution must depend only on the mapping it was handed"
    )
    assert first == second, (
        "settings resolution must depend only on the mapping it was handed"
    )


@pytest.mark.parametrize(
    "exported", ["/nonexistent/gh-output", ""], ids=["set", "empty"]
)
def test_workflow_output_is_read_as_a_step_of_its_own(exported: str) -> None:
    """Verify ``GITHUB_OUTPUT`` is resolved after everything else, and refused.

    Notes
    -----
    The value is deliberately absent from :func:`resolve_configuration`, and
    that is the contract: the shell expanded ``${GITHUB_OUTPUT:?}`` after it had
    created both artefacts, so a run that dies on this variable has already made
    them. Both arms of the ``:?`` form are read from the mapping, not the
    settings.
    """
    variables = {**BASELINE_ENVIRONMENT, OUTPUT_VARIABLE: exported}

    if exported:
        assert configuration.require_workflow_output(variables) == exported, (
            "a configured GITHUB_OUTPUT must be returned as its own spelling"
        )
        return
    with pytest.raises(configuration.ConfigurationError) as refusal:
        configuration.require_workflow_output(variables)

    assert str(refusal.value) == configuration.OUTPUT_DIAGNOSTIC, (
        "an empty GITHUB_OUTPUT must be refused with the shell's own sentence"
    )
