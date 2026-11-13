"""Record what the finalizer's two read-only stages observed.

``resolve`` records the candidate the package stage built, and ``snapshot``
records the release's draft flag, prerelease flag, and reported digests. Both
commands only write files a later step reads, and neither changes the release,
so their writers sit apart from the reads and writes that surround the one
edit. A reader asking what a refusal might have left on disk finds this module;
a reader asking what the run did to GitHub finds
``finalize_github_release_records``.

The candidate record is re-validated rather than copied on trust, because a
stage that recorded an identity by hand must not be able to reclassify the
release. ``resolve`` writes it once, and ``publish`` re-applies the same
agreement when it reads the record back, so the two stages cannot disagree
about which candidate this run is publishing.

Run via ``scripts/finalize_github_release.py``; behavioural coverage lives in
``scripts/tests/test_finalize_github_release.py``.
"""

import typing as typ

import finalize_github_release_io as release_io
import finalize_github_release_records as release_records
import release_candidate
import release_identity_record

if typ.TYPE_CHECKING:
    # The path annotations are lazy on the Python 3.14 baseline and nothing
    # here reads them at runtime, so importing the module would only cost
    # start-up on a path that every finalization run takes.
    import pathlib


def record_candidate(
    identity: pathlib.Path, *, tag: str, commit: str, output: pathlib.Path
) -> None:
    """Check the package record's identity, then record the candidate.

    The package record nests the identity under ``identity`` beside the crate
    name and digest; only the identity is this stage's concern, and it is
    re-validated so a hand-edited record cannot reclassify the release.

    Parameters
    ----------
    identity
        The package record ``package-crate`` retained.
    tag
        The release being finalized, which the record must name.
    commit
        The revision being published, which the record must name.
    output
        Where the candidate record is written for the later steps to read.

    Raises
    ------
    FinalizeError
        With :data:`EXIT_INPUT_UNREADABLE` when the record is not a package
        record or holds no usable identity, and :data:`EXIT_REFUSED` when it
        names another tag or revision. Both refuse before anything is written.

    Notes
    -----
    The tag-and-revision agreement is :func:`release_candidate.identity_refusal`,
    the same rule ``publish`` applies to the candidate record. One rule read
    twice cannot report the same disagreement two ways.

    Examples
    --------
    >>> import json
    >>> import pathlib
    >>> import tempfile
    >>> identity = {
    ...     "tag": "v1.2.3", "commit": "0" * 40, "package_name": "p",
    ...     "version": "1.2.3", "bin_name": "b", "is_prerelease": False,
    ... }
    >>> with tempfile.TemporaryDirectory() as directory:
    ...     source = pathlib.Path(directory) / "package.json"
    ...     _ = source.write_text(
    ...         json.dumps({"identity": identity}), encoding="utf-8"
    ...     )
    ...     output = pathlib.Path(directory) / "candidate.json"
    ...     record_candidate(source, tag="v1.2.3", commit="0" * 40, output=output)
    ...     json.loads(output.read_text(encoding="utf-8"))["version"]
    recorded candidate v1.2.3 at 0000000000000000000000000000000000000000
    '1.2.3'
    """
    envelope = release_io.read_json(identity)
    if not isinstance(envelope, dict) or not isinstance(envelope.get("identity"), dict):
        raise release_io.FinalizeError(
            release_io.EXIT_INPUT_UNREADABLE,
            f"{identity} is not a package record with an identity object",
        )
    try:
        recorded = release_identity_record.identity_from_mapping(envelope["identity"])
    except release_candidate.IdentityError as error:
        raise release_io.FinalizeError(
            release_io.EXIT_INPUT_UNREADABLE,
            f"{identity} is not a usable candidate: {error}",
        ) from error
    reason = release_candidate.identity_refusal(recorded, tag=tag, commit=commit)
    if reason is not None:
        raise release_io.FinalizeError(release_io.EXIT_REFUSED, reason)
    release_io.write_json(output, recorded.as_record())
    print(f"recorded candidate {recorded.tag} at {recorded.commit}")


def record_snapshot(
    tag: str,
    repository: str,
    *,
    state_path: pathlib.Path,
    assets_path: pathlib.Path,
) -> None:
    """Record the release's draft flag, prerelease flag, and reported digests.

    Parameters
    ----------
    tag
        The release to read.
    repository
        The ``OWNER/REPO`` the release belongs to.
    state_path
        Where the whole state record is written.
    assets_path
        Where the bare name-to-digest mapping is written for the planner, which
        has no business reading a draft flag.

    Notes
    -----
    A ``FinalizeError`` from :func:`snapshot_release` or either write
    propagates unchanged: :data:`EXIT_REMOTE_FAILED` when the release cannot be
    read, and :data:`EXIT_OUTPUT_UNWRITABLE` when a record cannot be written.
    This function only reads GitHub, so an interrupted snapshot never leaves
    the release in a different state.
    """
    state = release_records.snapshot_release(tag, repository)
    release_io.write_json(
        state_path,
        {
            "assets": state.assets,
            "isDraft": state.is_draft,
            "isPrerelease": state.is_prerelease,
            "tag": state.tag,
        },
    )
    release_io.write_json(assets_path, state.assets)
    print(
        f"release {tag}: draft={state.is_draft} "
        f"prerelease={state.is_prerelease} assets={len(state.assets)}"
    )
