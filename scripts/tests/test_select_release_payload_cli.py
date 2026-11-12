"""Test the selection CLI end to end over a real staged tree.

Each case builds the tree a release downloads -- one directory per producer,
hoisted archives at the root, and the build products that ship inside the
packages -- and asserts on what the CLI left behind. Asserting on the exit code
alone would pass for a run that refused everything, so every case also states
whether a payload exists and which names it holds.

The staging itself lives in ``select_release_payload_support``, so a case here
states only the contradiction it introduces and the evidence it expects.

Run via ``make test-release-publication``.
"""

import typing as typ

import pytest
import release_digest_files
import release_digest_record_files
import select_release_payload_cli as cli
from select_release_payload_origins_support import (
    ARM_ARCHIVE,
    ARM_DEB,
    ARM_RPM,
    X86_ARCHIVE,
    exchange_claims,
    two_target_tree,
)
from select_release_payload_support import (
    ARCHIVE,
    DEB,
    RETAINED,
    RPM,
    SIDECAR,
    Provenance,
    installed_names,
    published_path,
    record_provenance,
    run_cli,
    stage,
    staged_assets,
    staged_tree,
)

if typ.TYPE_CHECKING:
    import argparse
    import pathlib

    import release_payload_selection


def test_a_complete_tree_installs_exactly_the_declared_names(
    tmp_path: pathlib.Path,
) -> None:
    """Copy the declared assets under their published names and nothing else."""
    tree = staged_tree(tmp_path)
    assert run_cli(tree) == cli.EXIT_OK, "a complete tree must be accepted"
    assert installed_names(tree["output"]) == sorted([ARCHIVE, SIDECAR, DEB, RPM]), (
        "the payload must hold exactly the declared names"
    )


def test_the_trusted_record_describes_the_installed_bytes(
    tmp_path: pathlib.Path,
) -> None:
    """Write a record the verifier and finalizer can compare downloads against."""
    tree = staged_tree(tmp_path)
    assert run_cli(tree) == cli.EXIT_OK, "a complete tree must be accepted"
    record = release_digest_record_files.read_digest_record(tree["record"])
    assert sorted(record.names()) == sorted([ARCHIVE, SIDECAR, DEB, RPM]), (
        "the record must describe every declared name"
    )
    installed = tree["output"] / cli.PAYLOAD_DIRECTORY / DEB
    assert record.as_mapping()[DEB] == release_digest_files.digest_of_file(installed), (
        "the recorded digest must describe the installed bytes"
    )


def test_retained_material_never_reaches_the_payload(tmp_path: pathlib.Path) -> None:
    """Exclude the binary, licence, and completions the packages contain.

    Refusing them would reject a good build; uploading them would publish files
    the release never declared.
    """
    tree = staged_tree(tmp_path)
    assert run_cli(tree) == cli.EXIT_OK, "retained material must not be refused"
    recorded = release_digest_record_files.read_digest_record(tree["record"])
    for retained in RETAINED:
        assert retained not in installed_names(tree["output"]), retained
        assert retained not in recorded.names(), retained


def test_the_producer_record_is_not_uploaded_as_an_asset(
    tmp_path: pathlib.Path,
) -> None:
    """Leave the provenance record out of the payload it describes."""
    tree = staged_tree(tmp_path)
    assert run_cli(tree) == cli.EXIT_OK, "the record must not be an asset"
    assert "producer-provenance.json" not in installed_names(tree["output"]), (
        "the producer record must stay out of the payload"
    )


def refused_after_reprovenance(
    tmp_path: pathlib.Path, *, fields: Provenance, why: str
) -> None:
    """Stage a tree whose producer record differs only in ``fields``.

    Parameters
    ----------
    tmp_path
        The case's scratch directory.
    fields
        The producer-record fields the case is making hostile.
    why
        What the refusal proves, used as the assertion's message.

    Notes
    -----
    Both identity refusals share this shape: a complete, otherwise legitimate
    tree, with one field of its provenance rewritten to belong to a different
    commit or run. Only the field and the sentence differ, so the staging and
    the two assertions live here.
    """
    tree = staged_tree(tmp_path, producer=False)
    record_provenance(tree["dist"], fields=fields, assets=staged_assets(tree))
    assert run_cli(tree) == cli.EXIT_PAYLOAD_REFUSED, why
    assert not tree["output"].exists(), "a refusal must not leave a payload"


def test_two_correctly_attributed_producers_are_accepted(
    tmp_path: pathlib.Path,
) -> None:
    """Accept the control: each package sits below the subtree that claims it.

    Without this case the refusal below could be caused by the two-target
    configuration rather than by the moved package.
    """
    tree = two_target_tree(tmp_path)
    assert run_cli(tree) == cli.EXIT_OK, "two clean producers must be accepted"
    assert installed_names(tree["output"]) == sorted([
        X86_ARCHIVE,
        f"{X86_ARCHIVE}.sha256",
        ARM_ARCHIVE,
        f"{ARM_ARCHIVE}.sha256",
        DEB,
        RPM,
        ARM_DEB,
        ARM_RPM,
    ]), "the payload must hold both targets' declared names"


def test_a_package_moved_into_a_siblings_subtree_is_refused(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse a package the claiming record's own subtree did not stage.

    The x86 owner still claims ``DEB`` and the counts, formats, and union are
    all unchanged -- only the file's location moved, into the aarch64
    subtree. A check that stopped at count and format would accept this tree
    and publish a ``.deb`` whose bytes came out of another target's download.
    """
    tree = two_target_tree(tmp_path)
    moved = tree["dist"] / "linux-aarch64" / DEB
    moved.parent.mkdir(parents=True, exist_ok=True)
    _ = moved.write_bytes((tree["dist"] / "linux-x86_64" / DEB).read_bytes())
    _ = (tree["dist"] / "linux-x86_64" / DEB).unlink()
    assert run_cli(tree) == cli.EXIT_PAYLOAD_REFUSED, (
        "a package from a sibling's subtree must be refused"
    )
    assert not tree["output"].exists(), "a refusal must not leave a payload"


def test_two_records_exchanging_same_format_claims_are_refused(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse two records that trade their package claims one for one.

    A swap rather than a move, so every count, every format, the union of
    published names, and every digest are all exactly what an honest tree
    holds. Both records are equally wrong: each claims its sibling's
    same-format packages. Only the origin pairing separates that from a
    correct tree, which is why a count or digest check cannot see it.
    """
    tree = two_target_tree(tmp_path)
    exchange_claims(tree["dist"])
    assert run_cli(tree) == cli.EXIT_PAYLOAD_REFUSED, (
        "each record claims the other's packages, so both must be refused"
    )
    assert not tree["output"].exists(), "a refusal must not leave a payload"


def test_a_producer_from_another_commit_is_refused(tmp_path: pathlib.Path) -> None:
    """Refuse a tree left over from an earlier candidate."""
    refused_after_reprovenance(
        tmp_path, fields=Provenance(commit="b" * 40), why="a stale candidate was used"
    )


def test_a_producer_from_another_run_is_refused(tmp_path: pathlib.Path) -> None:
    """Refuse a record whose producer belongs to a different workflow run."""
    refused_after_reprovenance(
        tmp_path, fields=Provenance(run_id="9999"), why="a foreign run was used"
    )


def test_a_record_naming_an_unknown_platform_is_unusable_input(
    tmp_path: pathlib.Path,
) -> None:
    """Report a broken record as input trouble, not as a refused payload.

    Re-running publication cannot repair it; the producer job must be fixed.
    """
    tree = staged_tree(tmp_path, producer=False)
    record_provenance(
        tree["dist"], fields=Provenance(platform="beos"), assets=staged_assets(tree)
    )
    assert run_cli(tree) == cli.EXIT_UNUSABLE_INPUT, (
        "an unknown platform is input trouble, not a refusal"
    )
    assert not tree["output"].exists(), "a refusal must not leave a payload"


def test_an_ambiguous_declared_name_is_refused(tmp_path: pathlib.Path) -> None:
    """Refuse two staged files claiming one published name."""
    tree = staged_tree(tmp_path)
    stage(tree["dist"], f"stray/{DEB}")
    assert run_cli(tree) == cli.EXIT_PAYLOAD_REFUSED, (
        "an ambiguous name must be refused"
    )
    assert not tree["output"].exists(), "a refusal must not leave a payload"


def test_a_release_shaped_stray_is_refused(tmp_path: pathlib.Path) -> None:
    """Refuse an undeclared staged file that would publish if uploaded."""
    tree = staged_tree(tmp_path)
    stage(tree["dist"], "netsuke-9.9.9-aarch64-apple-darwin.tar.gz")
    assert run_cli(tree) == cli.EXIT_PAYLOAD_REFUSED, (
        "a release-shaped stray must be refused"
    )
    assert not tree["output"].exists(), "a refusal must not leave a payload"


def test_a_missing_declared_asset_is_refused(tmp_path: pathlib.Path) -> None:
    """Refuse a tree that lost one of the recorded assets."""
    tree = staged_tree(tmp_path)
    _ = published_path(tree["dist"], RPM).unlink()
    assert run_cli(tree) == cli.EXIT_PAYLOAD_REFUSED, (
        "a lost declared asset must be refused"
    )
    assert not tree["output"].exists(), "a refusal must not leave a payload"


def test_replaced_bytes_are_refused_before_the_copy(tmp_path: pathlib.Path) -> None:
    """Refuse a staged file whose bytes the producer did not record."""
    tree = staged_tree(tmp_path, producer=False)
    record_provenance(tree["dist"], assets=staged_assets(tree))
    _ = published_path(tree["dist"], DEB).write_bytes(b"rebuilt from another commit")
    assert run_cli(tree) == cli.EXIT_PAYLOAD_REFUSED, (
        "bytes the producer did not record must be refused"
    )
    assert not tree["output"].exists(), "a refusal must not leave a payload"


def test_a_source_swapped_after_the_digest_check_disagrees_with_the_record(
    tmp_path: pathlib.Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Record the approved digests, so a swap after the check stays visible.

    ``require_staged_digests`` and the copy are separate steps, and this case
    rewrites a source in the window between them. A record re-hashed at install
    time would describe whatever the swap left behind, so ``selection.json``,
    the declaration record, and the payload would all agree with each other and
    none of them with the producer. Recording the digests selection verified is
    what makes the payload disagree with the record, which is the disagreement
    the uploader refuses.

    ``select_payload`` is wrapped rather than ``copyfile``: the swap has to
    land after the digest check has returned and before the copy is asked for
    the bytes, and this is the seam that bounds that window.
    """
    tree = staged_tree(tmp_path)
    approved = staged_assets(tree)
    genuine_select = cli.select_payload

    def select_then_swap(
        arguments: argparse.Namespace,
    ) -> release_payload_selection.PayloadSelection[pathlib.Path]:
        """Run the real selection, then replace one source it approved."""
        selection = genuine_select(arguments)
        _ = published_path(tree["dist"], DEB).write_bytes(b"swapped after the check")
        return selection

    monkeypatch.setattr(cli, "select_payload", select_then_swap)
    assert run_cli(tree) == cli.EXIT_OK, "the install itself must still succeed"
    installed = tree["output"] / cli.PAYLOAD_DIRECTORY / DEB
    assert installed.read_bytes() == b"swapped after the check", (
        "the copy must read the replacement, or the case proves nothing"
    )
    recorded = release_digest_record_files.read_digest_record(tree["record"])
    assert recorded.as_mapping()[DEB] == approved[DEB], (
        "the record must keep the digests selection verified"
    )
    assert recorded.as_mapping()[DEB] != release_digest_files.digest_of_file(
        installed
    ), "the record must disagree with the bytes that were copied"


def test_a_missing_producer_record_is_refused(tmp_path: pathlib.Path) -> None:
    """Refuse a tree whose producers never reported, naming the target.

    The download lost a producer's record, so the tree is not this release's
    even though every file in it is readable.
    """
    tree = staged_tree(tmp_path, producer=False)
    assert run_cli(tree) == cli.EXIT_PAYLOAD_REFUSED, (
        "a producer that never reported must be refused"
    )
    assert not tree["output"].exists(), "a refusal must not leave a payload"


def test_an_existing_output_directory_is_unwritable_output(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse to merge into a directory left by an earlier attempt.

    The tree here is complete and this release's, so nothing is wrong with the
    input: only the destination refuses. Reporting ``EXIT_UNUSABLE_INPUT``
    would send an operator to the producer records for a fault that lives in
    the workspace.
    """
    tree = staged_tree(tmp_path)
    tree["output"].mkdir()
    assert run_cli(tree) == cli.EXIT_OUTPUT_UNWRITABLE, (
        "an existing output directory is a destination fault, not an input one"
    )


def test_a_malformed_staging_config_is_unusable_input(
    tmp_path: pathlib.Path,
) -> None:
    """Report a configuration that cannot declare any release."""
    tree = staged_tree(tmp_path)
    _ = tree["staging_config"].write_text("not = [toml", encoding="utf-8")
    assert run_cli(tree) == cli.EXIT_UNUSABLE_INPUT, (
        "a malformed config must be reported as input trouble"
    )
    assert not tree["output"].exists(), "a refusal must not leave a payload"


@pytest.mark.parametrize("name", [ARCHIVE, SIDECAR, DEB, RPM])
def test_every_declared_name_is_copied_from_its_own_staged_file(
    tmp_path: pathlib.Path, *, name: str
) -> None:
    """Copy each name's bytes, not one asset's bytes under every name."""
    tree = staged_tree(tmp_path)
    assert run_cli(tree) == cli.EXIT_OK, "a complete tree must be accepted"
    installed = tree["output"] / cli.PAYLOAD_DIRECTORY / name
    assert installed.read_bytes() == name.encode("utf-8"), (
        "each name must be copied from its own staged file"
    )
