"""Resolve the producer step's GitHub expressions into values a runner would.

The producer step is invoked by the shell, so everything it reads arrives
through ``env``. Those values are written as GitHub expressions, and a case
that ran the step without expanding them would hand the script literal
``${{ ... }}`` text: the recorder would then claim a commit named
``${{ inputs.commit }}`` and a case asserting "the record names the candidate"
would pass on nonsense.

This module resolves exactly the expressions the step and its job use, and
fails the case on any other, so an input added to the workflow shows up as an
explicit refusal rather than as an unset variable on a runner.

Run via ``make test-workflow-contracts``.
"""

import re

import upload_declared_assets_producer_tree as tree
import upload_declared_assets_staging as staging
from upload_declared_assets_staging import Facts, Target
from workflow_loading import (
    PACKAGE_WORKFLOW_PATH,
    job_steps,
    load_workflow,
    named_step,
    require_mapping,
    workflow_job,
)

#: The job in ``build-and-package.yml`` that owns the producer step.
BUILD_JOB = "build"

#: The step that hashes a producer's own artefacts and writes its record.
PRODUCER_STEP = "Record producer provenance"

#: The attempt the producer records. Fixed, because nothing compares it: a
#: retried job may legitimately hand on fresh bytes.
RUN_ATTEMPT = "1"

#: An ``inputs`` reference in either of the two spellings the workflows use.
_INPUT_DOTTED = re.compile(r"\A\$\{\{ inputs\.([a-z-]+) \}\}\Z")
_INPUT_INDEXED = re.compile(r"\A\$\{\{ inputs\['([a-z-]+)'\] \}\}\Z")

#: The ``cond && 'a' || ''`` conditional the producer's job environment
#: carries, which GitHub substitutes by evaluating ``cond``.
_PLATFORM_TERNARY = re.compile(
    r"\A\$\{\{ inputs\.platform == '([a-z]+)' && '([^']*)' \|\| '([^']*)' \}\}\Z"
)

#: The ``github`` context values the producer step names, by their spelling.
_GITHUB_CONTEXT = {
    "${{ github.run_id }}": staging.RUN_ID,
    "${{ github.run_attempt }}": RUN_ATTEMPT,
}

#: A staging or packaging action's step output, resolved by
#: :func:`upload_declared_assets_producer_tree.stage_paths`.
_STEP_OUTPUT = re.compile(r"\A\$\{\{ steps\.[a-z_]+\.outputs\['([a-z-]+)'\] \}\}\Z")


def producer_step() -> dict[str, object]:
    """Return the producer recording step as the workflow writes it.

    Returns
    -------
    dict[str, object]
        The step mapping, with its ``env`` block and ``run`` body.
    """
    steps = job_steps(load_workflow(PACKAGE_WORKFLOW_PATH), BUILD_JOB)
    return named_step(steps, PRODUCER_STEP)


def _literal(expression: str, *, facts: Facts, target: Target) -> str | None:
    """Return the fixed value one expression substitutes, when it has one.

    Returns
    -------
    str | None
        The substituted value, or ``None`` when a later resolver may claim the
        expression.
    """
    literals = {
        "${{ inputs.commit }}": staging.COMMIT,
        "${{ inputs.platform }}": target.platform,
        "${{ inputs['stage-target'] }}": target.key,
        "${{ inputs.version }}": facts.version,
        "${{ inputs['bin-name'] }}": facts.bin_name,
        "${{ inputs['artifact-suffix'] }}": staging.MACOS_SUFFIX.format(
            arch=target.arch
        ),
        **_GITHUB_CONTEXT,
    }
    return literals.get(expression)


def _input_name(expression: str) -> str | None:
    """Return the ``inputs`` name an expression reads, in either spelling.

    Returns
    -------
    str | None
        The input's name, or ``None`` when the expression reads another
        context.
    """
    for pattern in (_INPUT_DOTTED, _INPUT_INDEXED):
        match = pattern.match(expression)
        if match is not None:
            return match.group(1)
    return None


def _resolved_input(name: str, *, facts: Facts, target: Target) -> str:
    """Return the fixed value ``name`` carries, in whichever spelling holds one.

    Returns
    -------
    str
        The value the runner would substitute for that input.

    Raises
    ------
    AssertionError
        If the table has no entry under either spelling, which means the
        workflow grew an input this case has not been taught about.

    Notes
    -----
    Both spellings are asked for directly rather than one being respelled into
    the other and resolved again. The respelled form is itself an expression
    ``resolve`` claims, so an input with no entry would recurse on it forever
    instead of reaching the assertion this module promises.
    """
    for spelling in (f"${{{{ inputs.{name} }}}}", f"${{{{ inputs['{name}'] }}}}"):
        fixed = _literal(spelling, facts=facts, target=target)
        if fixed is not None:
            return fixed
    message = f"{PRODUCER_STEP} reads input {name!r}, which this case cannot resolve"
    raise AssertionError(message)


def resolve(expression: str, *, facts: Facts, target: Target) -> str:
    """Return the value GitHub would substitute for one step expression.

    Returns
    -------
    str
        The value the runner would substitute.

    Raises
    ------
    AssertionError
        The step names an expression this module cannot resolve, which means
        the workflow grew an input this case has not been taught about.
    """
    fixed = _literal(expression, facts=facts, target=target)
    if fixed is not None:
        return fixed
    if "${{" not in expression:
        # A plain value, such as the pinned shared-action revision the job
        # restates, is the same on every platform and needs no resolution.
        return expression
    name = _input_name(expression)
    if name is not None:
        return _resolved_input(name, facts=facts, target=target)
    output = _STEP_OUTPUT.match(expression)
    if output is not None:
        return tree.stage_paths(facts, target)[output.group(1)]
    ternary = _PLATFORM_TERNARY.match(expression)
    if ternary is not None:
        platform, when_true, when_false = ternary.groups()
        return when_true if platform == target.platform else when_false
    # Any other expression is one the workflow has grown since this case was
    # written; failing loudly beats substituting a value the runner would not.
    message = f"{PRODUCER_STEP} names an expression this case cannot resolve: "
    raise AssertionError(f"{message}{expression}")


def step_environment(facts: Facts, target: Target) -> dict[str, str]:
    """Return the producer step's environment with every expression resolved.

    The step inherits the job's environment, so both blocks are resolved: the
    macOS package name interpolates ``BIN_NAME`` and ``VERSION`` from the job,
    while the record's identity comes from the step's own ``env``.

    Returns
    -------
    dict[str, str]
        The step's environment variables.
    """
    workflow = load_workflow(PACKAGE_WORKFLOW_PATH)
    blocks = (
        require_mapping(
            workflow_job(workflow, BUILD_JOB).get("env"), f"{BUILD_JOB} job env"
        ),
        require_mapping(producer_step().get("env"), f"{PRODUCER_STEP} env"),
    )
    return {
        str(key): resolve(str(value), facts=facts, target=target)
        for block in blocks
        for key, value in block.items()
    }
