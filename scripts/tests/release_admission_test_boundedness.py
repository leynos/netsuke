r"""Run the gate under generated identifiers and assert what they reached.

Split from :mod:`test_release_admission_metric_boundedness` when that module
reached the project's file-size ceiling. The seam is the kind of code each half
carries: this module owns the *mechanism* -- the strategies that generate
identifiers, the doubles' answer to a workflow-run query, the run itself, and
the four claims the property makes about one run -- while its sibling owns the
two tests and the argument that explains what each is for. A reader wanting to
know *why* the property exists reads the test module; a reader wanting to know
*what it does* reads this one.

The generated identifiers all carry :data:`IDENTIFIER_MARKER` at both ends, for
two reasons that are argued in full on that constant: it keeps them outside the
gate's own `[a-z_]` vocabulary so a substring collision cannot be mistaken for a
leak, and it keeps them unchanged by the newline strip production applies to the
API's answer before storing it.
"""

import pathlib
import tempfile
import typing as typ

from cmd_mox import CmdMox
from hypothesis import strategies as st
from release_admission_test_scenarios import is_workflow_run_query
from release_admission_test_support import (
    GITHUB_REPOSITORY,
    METRICS_VALIDATOR,
    OPERATION_SEQUENCE,
    Boundaries,
    GateRun,
    Handler,
    RunShape,
    assert_complete_records,
    assert_identifiers_excluded_from_diagnostics,
    assert_identifiers_excluded_from_records,
    install_boundaries,
    run_gate,
)

if typ.TYPE_CHECKING:
    from cmd_mox.ipc import Invocation

#: The environment variables the property uses to carry generated identifiers
#: that are not the revision. Production never reads these names -- there is no
#: consumer of either anywhere under ``.github/scripts`` -- so they prove
#: transport only, and the claim they support is exactly that: a value that
#: reaches the child's environment does not reach a record or a diagnostic.
#: They are named as *scenario* variables rather than as configuration so a
#: reader is not misled into thinking the gate has inputs the production code
#: does not read.
IDENTIFIER_PATH_VARIABLE = "NETSUKE_RELEASE_ADMISSION_SCENARIO_PATH"
IDENTIFIER_URL_VARIABLE = "NETSUKE_RELEASE_ADMISSION_SCENARIO_URL"

#: A character no label value or metric name contains. **Every fixed token the
#: gate can emit is written in ``[a-z_]`` alone** -- the five operation names,
#: ``success``, ``failure``, ``unknown``, ``none``, the canaries, the error
#: categories, the trace events, and the three ``netsuke_release_admission_*``
#: metric names -- so a character outside that set occurs in no emitted token,
#: and an identifier that contains one therefore cannot be a substring of any
#: of them.
#:
#: Without this, the exclusion assertion stops being a claim about identifiers
#: and becomes a claim about the gate's vocabulary: a one-character identifier is
#: a substring of nearly every label it is compared against. Hypothesis found
#: that as soon as the run-id restoration put the identifier back into the
#: search -- example ``run_id='_'`` matched ``resolve_tag_commit``, and the
#: property failed on a run where the gate had leaked nothing. A length bound is
#: not enough on its own: ``none`` is four characters and is a label, and
#: ``failure`` is seven.
#:
#: The digit also keeps the generated identifier on the far side of the runtime's
#: fixed vocabulary *by construction* rather than by sampling, which is what a
#: hypothesis example has to be able to state. It costs nothing the property was
#: testing: the domain is still arbitrary Unicode payloads within the declared
#: bound, and the digits are inside it. The bound is the strategy's, not the
#: vocabulary's -- payloads run 4 to 31 characters, so a generated identifier is
#: 6 to 33 characters once both markers are added, and a value outside that range
#: is not sampled rather than excluded by the marker.
#:
#: The marker is written at **both ends** for a second reason. An identifier
#: that reached a record only after some normalisation would be searched for in
#: the wrong form, and the difference runs in the dangerous direction: a leak of
#: the normalised value would not match the raw one and the exclusion would
#: pass. Symmetric markers make any such normalisation visible as a mismatch
#: rather than silent, and they rule out the degenerate all-newline value,
#: whose normalised form is the empty string, a substring of every label.
IDENTIFIER_MARKER = "7"
#: The smallest number of generated characters an identifier carries. Kept above
#: one so an identifier is never a single marked character, which would still be
#: a shape the gate should be tested against but says less than a longer one.
IDENTIFIER_MINIMUM_LENGTH = 4

#: Text that includes a character no emitted token contains, so a value it
#: generates cannot be a substring of the gate's own vocabulary, and that ends in
#: that character so production's newline strip cannot change it. Surrogates are
#: excluded because the record is written as UTF-8, and NUL because the child's
#: environment cannot carry one.
IDENTIFIER_TEXT = st.text(
    alphabet=st.characters(blacklist_categories=("Cs",), blacklist_characters="\x00"),
    min_size=IDENTIFIER_MINIMUM_LENGTH,
    max_size=31,
).map(lambda value: f"{IDENTIFIER_MARKER}{value}{IDENTIFIER_MARKER}")
#: Canonical Git object IDs, so a generated revision reaches the commit and
#: workflow-run queries instead of being refused as malformed first. Only the
#: SHA-1 width is generated: the manifest schema states its ``commit`` as a
#: 40-hex object ID, so no document could admit a wider one and the property
#: would be asserting a refusal rather than an admission.
GIT_OBJECT_ID = st.text(alphabet="0123456789abcdef", min_size=40, max_size=40)

#: The filter the workflow-run request applies, split out so the property names
#: the variable part of each request exactly once.
RUN_QUERY_FILTER = ".head_sha // empty"


def answer_run_head(revision: str) -> Handler:
    """Return a handler that reports *revision* as the running run's head.

    The answer is what makes the run lookup agree with the candidate, so the
    gate reaches the evidence operations rather than stopping at a candidate
    mismatch.

    Returns
    -------
    Handler
        An answer for the run query that carries the candidate revision.
    """

    def answer(_invocation: Invocation) -> tuple[str, str, int]:
        """Succeed with the candidate as the run's head."""
        return f"{revision}\n", "", 0

    return answer


def run_under_generated_identifiers(
    revision: str, run_id: str, path: str, url: str
) -> tuple[GateRun, Boundaries]:
    """Run the gate once with the given identifiers in play.

    A controller is built per example rather than taken from the fixture: the
    doubles must be registered before replay, and Hypothesis runs every example
    inside one test function, so one fixture instance cannot serve them all.

    The manifest the harness writes names the *generated* candidate, because it
    is built from the environment the child actually inherits. That is what
    leaves this a run about identifiers rather than a run about a mismatch.

    Parameters
    ----------
    revision
        The canonical object ID the workflow exports and the queries must carry.
    run_id
        Generated identifier the child's ``GITHUB_RUN_ID`` names, so the value
        reaches the run request through production.
    path, url
        Generated identifiers carried in the child's environment only. Nothing
        in production reads either name; the values exist to be transported.

    Returns
    -------
    tuple[GateRun, Boundaries]
        The completed run and the doubles whose recorded calls answer whether
        the identifiers crossed any boundary they should not have.
    """
    with (
        tempfile.TemporaryDirectory() as directory_name,
        CmdMox(verify_on_exit=False) as cmd_mox,
    ):
        boundaries = install_boundaries(cmd_mox)
        boundaries.gh.wraps(
            lambda invocation, handler: (
                answer_run_head(revision)(invocation)
                if is_workflow_run_query(invocation)
                else handler(invocation)
            )
        )
        cmd_mox.replay()
        run = run_gate(
            cmd_mox,
            pathlib.Path(directory_name),
            evidence_state="fresh",
            extra_environment={
                "GITHUB_SHA": revision,
                "GITHUB_RUN_ID": run_id,
                "NETSUKE_RELEASE_ADMISSION_ENFORCE": "false",
                IDENTIFIER_PATH_VARIABLE: f"path-{path}",
                IDENTIFIER_URL_VARIABLE: f"url-{url}",
            },
        )
        cmd_mox.verify()
    return run, boundaries


def assert_gate_run_is_valid(run: GateRun) -> None:
    """Assert the run admitted the generated candidate under the fixed schema."""
    assert run.result.returncode == 0, run.result.stderr
    METRICS_VALIDATOR.validate_metrics(run.metrics)
    METRICS_VALIDATOR.validate_traces(run.traces)
    assert run.outputs["gate-outcome"] == "success", (
        "the generated candidate must be admitted by its own manifest"
    )


def assert_identifiers_reached_the_boundaries(
    boundaries: Boundaries, revision: str, run_id: str
) -> None:
    """Assert both doubled boundaries were asked about the generated identifiers."""
    expected_requests = [
        ["api", f"repos/{GITHUB_REPOSITORY}/commits/{revision}", "--jq", ".sha"],
        [
            "api",
            f"repos/{GITHUB_REPOSITORY}/actions/runs/{run_id}",
            "--jq",
            RUN_QUERY_FILTER,
        ],
    ]
    assert boundaries.gh.calls == expected_requests, (
        "the exact admission requests must carry the generated identifiers"
    )
    assert boundaries.git.calls == [
        ["fetch", "--depth", "1", "--no-tags", "origin", "--", revision]
    ], "the bounded fetch must carry the generated revision"


def assert_identifiers_crossed_only_the_environment(
    boundaries: Boundaries, path: str, url: str
) -> None:
    """Assert the environment-only identifiers reached the child, and no further."""
    crossed = {
        (values.get(IDENTIFIER_PATH_VARIABLE), values.get(IDENTIFIER_URL_VARIABLE))
        for values in boundaries.git.envs
    }
    assert crossed == {(f"path-{path}", f"url-{url}")}, (
        "generated identifiers must cross the subprocess boundary in the environment"
    )


def assert_identifiers_never_reached_the_records(
    run: GateRun, identifiers: set[str]
) -> None:
    """Assert the run wrote complete records, and that none carries an identifier.

    Notes
    -----
    The records are shown to exist *first*. Both the shared validators and the
    exclusion helper are satisfied by an empty list, so without the shape
    assertion the exclusions below would hold vacuously on a gate that wrote
    nothing at all — which is the one way an exclusion claim can pass while
    saying nothing about the gate.
    """
    assert_complete_records(
        run.metrics,
        run.traces,
        RunShape(OPERATION_SEQUENCE, "success", "none"),
    )
    assert_identifiers_excluded_from_records(
        run.metrics, run.traces, identifiers, subject="generated identifiers"
    )
    assert_identifiers_excluded_from_diagnostics(
        run.result.stderr, identifiers, subject="generated identifiers"
    )
