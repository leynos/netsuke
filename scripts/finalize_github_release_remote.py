"""Perform the only two GitHub calls a finalizing run makes.

One read, one edit, and no third call. Both are wrapped here so the decision
layer receives values: a :class:`finalize_github_release_ports.ReleaseRead`
carries either the decoded state or the reason it could not be decoded, and a
:class:`finalize_github_release_ports.RemoteAnswer` carries what the edit
replied. A non-zero status is an answer, not an exception, because the decision
about what it means belongs to
:mod:`finalize_github_release_decisions`, and a test can state that decision
without starting a process.

The process itself is the injected ``runner``, and it has no default. In
production the command layer supplies
:func:`finalize_github_release_io.run_gh`, which launches ``gh`` through
Cuprum's allowlisted catalogue; a test supplies a function returning recorded
answers. That keeps the argv construction here, where it can be asserted token
by token, and the decision elsewhere, where it needs no process at all. The tag
and repository that pin a call to one release are fixed here too, by
:func:`reader_port` and :func:`editor_port`, so the orchestrator receives
callables it can only call and never assembles a token of the call itself.

Run via ``scripts/finalize_github_release.py``; behavioural coverage lives in
``scripts/tests/test_finalize_github_release_publish_ports.py``.
"""

import json

import finalize_github_release_io as release_io
import finalize_github_release_ports as release_ports
import finalize_github_release_state as release_state

VIEW_COMMAND = "gh release view"
EDIT_COMMAND = "gh release edit"


def _failed(command: str, exit_code: int, stderr: str | None) -> str:
    r"""Return the clause naming a command that did not succeed.

    Parameters
    ----------
    command
        The command as it was written, so the operator can recognise it.
    exit_code
        The status the command reported.
    stderr
        Whatever the command wrote to standard error, kept verbatim so its own
        wording reaches the operator rather than a paraphrase.

    Returns
    -------
    str
        One clause, with the diagnostic appended only when there was one.

    Examples
    --------
    >>> _failed("gh release view", 5, "HTTP 403\n")
    'gh release view exited 5: HTTP 403'
    >>> _failed("gh release view", 5, "")
    'gh release view exited 5'
    """
    detail = f"{command} exited {exit_code}"
    return f"{detail}: {stderr.strip()}" if stderr else detail


def read_release(
    tag: str,
    repository: str,
    *,
    runner: release_io.Runner = release_io.run_gh,
) -> release_ports.ReleaseRead:
    """Read GitHub's current view of the release, decoded or explained.

    Parameters
    ----------
    tag
        The release to read.
    repository
        The ``OWNER/REPO`` the release belongs to. A fork's release is not the
        upstream's, so it is never inferred from the checkout.
    runner
        The process effect that answers the read.

    Returns
    -------
    release_ports.ReleaseRead
        The decoded state, or the reason the response established none.

    Notes
    -----
    A launch failure still raises
    :class:`finalize_github_release_io.FinalizeError`: no answer was produced
    at all, which is a different fact from a command that answered with a
    failure. Everything the command did answer comes back as a value.

    Examples
    --------
    >>> import types
    >>> payload = (
    ...     '{"tagName": "v1.2.3", "isDraft": true, '
    ...     '"isPrerelease": false, "assets": []}'
    ... )
    >>> answer = types.SimpleNamespace(stdout=payload, stderr="", exit_code=0)
    >>> read_release("v1.2.3", "o/r", runner=lambda *a, **k: answer).state.is_draft
    True
    """
    result = runner(
        "release",
        "view",
        tag,
        "--repo",
        repository,
        "--json",
        release_io.VIEW_FIELDS,
    )
    if result.exit_code != 0:
        return release_ports.ReleaseRead(
            detail=_failed(VIEW_COMMAND, result.exit_code, result.stderr)
        )
    try:
        payload = json.loads(result.stdout or "")
        return release_ports.ReleaseRead(
            state=release_state.parse_release_state(payload, tag=tag)
        )
    except (json.JSONDecodeError, release_state.FinalizePolicyError) as error:
        return release_ports.ReleaseRead(
            detail=f"{VIEW_COMMAND} returned an unusable response: {error}"
        )


def edit_release(
    tag: str,
    repository: str,
    *,
    is_prerelease: bool,
    runner: release_io.Runner = release_io.run_gh,
) -> release_ports.RemoteAnswer:
    """Un-draft the release with the candidate's own prerelease flag.

    Parameters
    ----------
    tag
        The release to un-draft.
    repository
        The ``OWNER/REPO`` the release belongs to.
    is_prerelease
        The flag the validated candidate requires, never the live release's.
    runner
        The process effect that performs the edit. The production runner
        echoes this call, so the workflow log names what changed the release.

    Returns
    -------
    release_ports.RemoteAnswer
        Empty when ``gh`` accepted the edit, otherwise its status and
        diagnostic. A refused edit is an answer the decision reports, not an
        exception raised here.

    Examples
    --------
    >>> import types
    >>> answer = types.SimpleNamespace(stdout="", stderr="", exit_code=0)
    >>> def runner(*_args, **_kwargs):
    ...     return answer
    >>> edit_release("v1.2.3", "o/r", is_prerelease=False, runner=runner)
    RemoteAnswer(exit_code=0, detail='')
    """
    flag = "true" if is_prerelease else "false"
    result = runner(
        "release",
        "edit",
        tag,
        "--repo",
        repository,
        "--draft=false",
        f"--prerelease={flag}",
        echo=True,
    )
    if result.exit_code == 0:
        return release_ports.RemoteAnswer()
    return release_ports.RemoteAnswer(
        exit_code=result.exit_code,
        detail=_failed(EDIT_COMMAND, result.exit_code, result.stderr),
    )


def reader_port(
    tag: str | None, repository: str, *, runner: release_io.Runner
) -> release_ports.ReleaseReader:
    """Bind the release read to one tag and one repository.

    Parameters
    ----------
    tag
        The release to read. ``require_candidate`` refuses a tagless identity
        before this is asked, so the ``None`` branch is defensive only.
    repository
        The ``OWNER/REPO`` the release belongs to.
    runner
        The process effect that answers the read.

    Returns
    -------
    release_ports.ReleaseReader
        A callable taking only the ``after_edit`` fact, so the decision layer
        names no argument vector.

    Raises
    ------
    ValueError
        If no tag is bound, which no validated candidate can reach.

    Notes
    -----
    The two facts that pin a call to a release are fixed here, at the edge,
    rather than inside :func:`finalize_github_release_operations.publish_release`.
    The orchestrator therefore receives a port it can only call, which is what
    keeps the process dependency out of the module that decides.

    Examples
    --------
    >>> import types
    >>> payload = (
    ...     '{"tagName": "v1.2.3", "isDraft": true, '
    ...     '"isPrerelease": false, "assets": []}'
    ... )
    >>> answer = types.SimpleNamespace(stdout=payload, stderr="", exit_code=0)
    >>> read = reader_port("v1.2.3", "o/r", runner=lambda *a, **k: answer)
    >>> read(after_edit=False).state.is_draft
    True
    """
    if tag is None:
        msg = "a release read needs a tag"
        raise ValueError(msg)

    def read(*, after_edit: bool) -> release_ports.ReleaseRead:
        """Read the bound release."""
        del after_edit  # A read answers the same way whenever it is asked.
        return read_release(tag, repository, runner=runner)

    return read


def editor_port(
    tag: str | None, repository: str, *, runner: release_io.Runner
) -> release_ports.ReleaseEditor:
    """Bind the un-drafting edit to one tag and one repository.

    Parameters
    ----------
    tag
        The release to un-draft, already validated against the candidate.
    repository
        The ``OWNER/REPO`` the release belongs to.
    runner
        The process effect that performs the edit.

    Returns
    -------
    release_ports.ReleaseEditor
        A callable taking only the prerelease flag, so the flag is the only
        decision the orchestrator has to make about the call.

    Raises
    ------
    ValueError
        If no tag is bound, which no validated candidate can reach.

    Notes
    -----
    Binding the call here leaves the decision layer with "may this release be
    un-drafted", and every token that addresses the release with the adapter
    that already owns the only write this feature performs.

    Examples
    --------
    >>> import types
    >>> calls = []
    >>> def runner(*args, **kwargs):
    ...     calls.append(args)
    ...     return types.SimpleNamespace(stdout="", stderr="", exit_code=0)
    >>> edit = editor_port("v1.2.3", "o/r", runner=runner)
    >>> edit(is_prerelease=True).exit_code
    0
    >>> calls[0][:3]
    ('release', 'edit', 'v1.2.3')
    """
    if tag is None:
        msg = "a release edit needs a tag"
        raise ValueError(msg)

    def edit(*, is_prerelease: bool) -> release_ports.RemoteAnswer:
        """Un-draft the bound release."""
        return edit_release(tag, repository, is_prerelease=is_prerelease, runner=runner)

    return edit


def unknown_state_message(*, after_edit: bool) -> str:
    """Name what a failed GitHub call leaves established about the release.

    Parameters
    ----------
    after_edit
        Whether an un-drafting call had already been attempted.

    Returns
    -------
    str
        The sentence appended to a failure diagnosis. Neither case reports the
        release as a draft: a retry can begin after an earlier attempt already
        published it, and an operator told it was a draft might edit it by
        hand. The two cases still differ, because an attempted edit may already
        have landed even though the response was lost.

    Examples
    --------
    >>> "edit was attempted" in unknown_state_message(after_edit=True)
    True
    >>> "has not edited the release" in unknown_state_message(after_edit=False)
    True
    """
    if after_edit:
        return (
            "The release state is unknown: an edit was attempted, so it may "
            "already be public. Inspect the release, or re-run the finalizer, "
            "which records an already-public release without editing it again"
        )
    return (
        "This run has not edited the release, but its state is unknown: the "
        "read that would establish it failed. Inspect the release, or re-run "
        "the finalizer, which records an already-public release without "
        "editing it again"
    )
