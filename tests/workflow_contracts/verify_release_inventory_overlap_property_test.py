"""Vary the verifier's paths and hold its overlap rule to account.

The validation step refuses a run whose paths overlap, and it compares
*resolved* paths rather than the text the caller wrote, because a symbolic link
or an equality the containment test is not written for would otherwise slip
through. Each property below generates a directory name, arranges one relation
between the inputs, and asserts the verdict: a refusal that names the overlap
and never reaches the network, or — for the controls — an accepted run that
writes its record. Without the controls, a validation step that refused every
path would satisfy the refusals and still be wrong.

The hooks that stage each relation run inside the scenario's ``prepare`` hook,
because the harness stages the trusted tree at a fixed path before any hook
runs. A case that needs the tree elsewhere therefore moves it, rather than
restating how it is written.

The cases that vary one declared input are in
``verify_release_inventory_refusal_test.py``.

Run via ``make test-workflow-contracts``.
"""

import dataclasses

# Hypothesis resolves the annotations of a ``@given`` test at collection time,
# so this import has to exist at runtime as well as for the type checker.
import pathlib  # ruff: ignore[typing-only-standard-library-import]
import typing as typ

from hypothesis import HealthCheck, given, settings
from hypothesis import strategies as st
from verify_release_inventory_cases import default_scenario, final_diagnostic
from verify_release_inventory_harness import execute
from verify_release_inventory_scenario import Inputs

if typ.TYPE_CHECKING:
    import collections.abc as cabc

    from verify_release_inventory_harness import ActionRun


#: A directory name the property cases generate. The alphabet is deliberately
#: narrow: the point is that the refusal holds for *any* name, not that it
#: handles exotic ones, and a bounded alphabet keeps each example cheap.
_DIRECTORY_NAME = st.from_regex(r"[a-z][a-z0-9]{0,7}", fullmatch=True)

#: Hypothesis does not reset a function-scoped fixture between examples. Each
#: example runs in its own ``runs/run-N`` directory inside the fixture's tree,
#: so sharing the fixture is deliberate: the examples must not see each other's
#: workspace, and they do not.
PROPERTY_SETTINGS = settings(
    max_examples=8,
    deadline=None,
    suppress_health_check=[HealthCheck.function_scoped_fixture],
)


def _link(directory: str, target: str) -> cabc.Callable[[pathlib.Path], None]:
    """Return a hook that makes ``directory`` a symlink to ``target``.

    Returns
    -------
    cabc.Callable[[pathlib.Path], None]
        A hook taking the staged workspace.
    """

    def link(workspace: pathlib.Path) -> None:
        """Point the directory at the trusted tree through a symlink."""
        (workspace / directory).symlink_to(target, target_is_directory=True)

    return link


def _link_to_empty(directory: str, target: str) -> cabc.Callable[[pathlib.Path], None]:
    """Return a hook that creates ``target`` and links ``directory`` to it.

    Returns
    -------
    cabc.Callable[[pathlib.Path], None]
        A hook taking the staged workspace.

    Notes
    -----
    The target is created empty, because a symlink to a path that does not
    exist is a link the steps would be refusing for the wrong reason. The
    directory is empty rather than absent so the run is otherwise the matching
    default: an occupied download directory is a fault of its own.
    """

    def link(workspace: pathlib.Path) -> None:
        """Create the target directory and point the alias at it."""
        (workspace / target).mkdir()
        (workspace / directory).symlink_to(target, target_is_directory=True)

    return link


def _relocate_trusted(parent: str) -> cabc.Callable[[pathlib.Path], None]:
    """Return a hook that moves the staged trusted tree under ``parent``.

    The harness stages the trusted tree at a fixed path, so a case that needs
    the tree somewhere else moves it after staging rather than restating how it
    is written.

    Returns
    -------
    cabc.Callable[[pathlib.Path], None]
        A hook taking the staged workspace.
    """

    def relocate(workspace: pathlib.Path) -> None:
        """Move the trusted tree into the generated parent directory."""
        destination = workspace / parent
        destination.mkdir()
        (workspace / "trusted").rename(destination / "trusted")

    return relocate


def _trusted_at(parent: str) -> Inputs:
    """Return inputs naming the trusted tree once it has been relocated.

    Returns
    -------
    Inputs
        Inputs whose trusted paths point inside ``parent``, with every other
        value left at its default.
    """
    root = f"{parent}/trusted"
    return Inputs(
        trusted_directory=root,
        declaration_file=f"{root}/declaration.txt",
        trusted_record_file=f"{root}/trusted.json",
    )


def _escape_declaration(destination: str) -> cabc.Callable[[pathlib.Path], None]:
    """Return a hook that replaces the declaration with a link pointing out.

    Returns
    -------
    cabc.Callable[[pathlib.Path], None]
        A hook taking the staged workspace.
    """

    def escape(workspace: pathlib.Path) -> None:
        """Point the trusted declaration at a file outside the trusted tree."""
        outside = workspace / destination
        _ = outside.write_text("not the declaration\n", encoding="utf-8")
        link = workspace / "trusted" / "declaration.txt"
        link.unlink()
        link.symlink_to(outside)

    return escape


def _assert_refused(run: ActionRun) -> None:
    """Require the run to stop at the validation step without a network call."""
    assert len(run.completed) == 1, "no later step may run on a refused input"
    assert not run.gh_log, "a refused input must not reach the network"


def _assert_disjoint_refusal(run: ActionRun) -> None:
    """Require the run to be refused for overlapping paths."""
    _assert_refused(run)
    assert "must be disjoint" in final_diagnostic(run), final_diagnostic(run)


def _assert_accepted(run: ActionRun) -> None:
    """Require the run to complete and write the verified record."""
    assert run.completed[-1].returncode == 0, final_diagnostic(run)
    assert run.verified_record, "an accepted release must write its record"


@PROPERTY_SETTINGS
@given(name=_DIRECTORY_NAME)
def test_a_download_directory_inside_the_trusted_tree_is_refused(
    tmp_path: pathlib.Path, name: str
) -> None:
    """Refuse any download directory nested inside the trusted tree.

    The property is over the name rather than a fixed example: whatever the
    directory is called, a download that lands inside the trusted tree would
    let a downloaded file stand in for a trusted input.
    """
    scenario = default_scenario(inputs=Inputs(download_directory=f"trusted/{name}"))
    _assert_disjoint_refusal(execute(tmp_path, scenario))


@PROPERTY_SETTINGS
@given(name=_DIRECTORY_NAME)
def test_an_output_path_inside_the_download_directory_is_refused(
    tmp_path: pathlib.Path, name: str
) -> None:
    """Refuse any output path written into the download directory.

    The verifier walks the download directory to build the observed inventory,
    so a record written there would be counted as an attachment of the release
    it is meant to judge.
    """
    scenario = default_scenario(
        inputs=Inputs(output_path=f"release-download/{name}.json")
    )
    _assert_disjoint_refusal(execute(tmp_path, scenario))


@PROPERTY_SETTINGS
@given(name=_DIRECTORY_NAME)
def test_a_download_directory_beside_the_trusted_tree_is_accepted(
    tmp_path: pathlib.Path, name: str
) -> None:
    """Accept any download directory that is disjoint from the trusted tree.

    This is the control for the two refusals above: without it, a validation
    step that refused every path would satisfy them and still be wrong.
    """
    scenario = default_scenario(inputs=Inputs(download_directory=f"downloads/{name}"))
    _assert_accepted(execute(tmp_path, scenario))


@PROPERTY_SETTINGS
@given(name=_DIRECTORY_NAME)
def test_a_symlink_to_the_trusted_tree_is_refused(
    tmp_path: pathlib.Path, name: str
) -> None:
    """Refuse a download directory that only reaches the trusted tree by link.

    The comparison is made on resolved paths, so a symlink cannot smuggle the
    download directory inside the trusted tree while looking disjoint.
    """
    scenario = default_scenario(
        inputs=Inputs(download_directory=name),
        prepare=_link(name, "trusted"),
    )
    _assert_disjoint_refusal(execute(tmp_path, scenario))


@PROPERTY_SETTINGS
@given(name=_DIRECTORY_NAME)
def test_a_download_directory_above_the_trusted_tree_is_refused(
    tmp_path: pathlib.Path, name: str
) -> None:
    """Refuse a download directory the trusted tree is nested inside.

    Nesting is not a sibling hazard: a download placed above the trusted tree
    lets the download step write over the trusted tree's parent, so the
    comparison has to catch the ancestor relation in both directions.
    """
    parent = f"outer-{name}"
    scenario = default_scenario(
        inputs=dataclasses.replace(_trusted_at(parent), download_directory=parent),
        prepare=_relocate_trusted(parent),
    )
    _assert_disjoint_refusal(execute(tmp_path, scenario))


@PROPERTY_SETTINGS
@given(name=_DIRECTORY_NAME)
def test_an_output_path_equal_to_the_trusted_directory_is_refused(
    tmp_path: pathlib.Path, name: str
) -> None:
    """Refuse an output path that is the trusted directory itself.

    Equality is the case a containment test written only as "is the child under
    the parent" would miss, because the canonical paths are identical rather
    than merely related.
    """
    parent = f"outer-{name}"
    scenario = default_scenario(
        inputs=dataclasses.replace(
            _trusted_at(parent), output_path=f"{parent}/trusted"
        ),
        prepare=_relocate_trusted(parent),
    )
    _assert_disjoint_refusal(execute(tmp_path, scenario))


@PROPERTY_SETTINGS
@given(name=_DIRECTORY_NAME)
def test_an_output_path_equal_to_the_download_directory_is_refused(
    tmp_path: pathlib.Path, name: str
) -> None:
    """Refuse an output path that is the download directory itself."""
    download = f"download-{name}"
    scenario = default_scenario(
        inputs=Inputs(download_directory=download, output_path=download)
    )
    _assert_disjoint_refusal(execute(tmp_path, scenario))


@PROPERTY_SETTINGS
@given(name=_DIRECTORY_NAME)
def test_a_sibling_sharing_a_name_prefix_is_accepted(
    tmp_path: pathlib.Path, name: str
) -> None:
    """Accept a sibling whose name merely starts with the trusted tree's.

    This is the control for the disjointness refusals: a comparison written as
    a bare string prefix would refuse ``trusted-<anything>``, which is a
    separate directory and a legitimate place to download into.
    """
    scenario = default_scenario(inputs=Inputs(download_directory=f"trusted-{name}"))
    _assert_accepted(execute(tmp_path, scenario))


@PROPERTY_SETTINGS
@given(name=_DIRECTORY_NAME)
def test_an_output_path_through_a_link_into_the_trusted_tree_is_refused(
    tmp_path: pathlib.Path, name: str
) -> None:
    """Refuse an output path whose parent only reaches the trusted tree by link.

    The output path's own name says nothing about where the write lands, so the
    comparison is made on the resolved parent, not on the path as written.
    """
    alias = f"via-{name}"
    scenario = default_scenario(
        inputs=Inputs(output_path=f"{alias}/verified.json"),
        prepare=_link(alias, "trusted"),
    )
    _assert_disjoint_refusal(execute(tmp_path, scenario))


@PROPERTY_SETTINGS
@given(name=_DIRECTORY_NAME)
def test_a_trusted_directory_linked_onto_the_download_directory_is_refused(
    tmp_path: pathlib.Path, name: str
) -> None:
    """Refuse a trusted directory that resolves onto the download directory.

    The two paths are written differently and resolve to the same directory, so
    a verified record could be confirmed against bytes the release had just
    downloaded: the one substitution this job exists to prevent.
    """
    download = f"existing-{name}"
    alias = f"alias-{name}"
    inputs = Inputs(
        trusted_directory=alias,
        declaration_file=f"{alias}/declaration.txt",
        trusted_record_file=f"{alias}/trusted.json",
        download_directory=download,
    )
    scenario = default_scenario(
        inputs=inputs,
        prepare=_link_to_empty(alias, download),
    )
    _assert_disjoint_refusal(execute(tmp_path, scenario))


@PROPERTY_SETTINGS
@given(name=_DIRECTORY_NAME)
def test_a_declaration_reaching_outside_through_a_link_is_refused(
    tmp_path: pathlib.Path, name: str
) -> None:
    """Refuse a declaration that is inside the trusted tree only as written.

    The verifier compares the downloaded bytes against this file, so a link
    that resolves out of the trusted tree would let an untrusted file stand in
    for the publisher's declaration while the path still reads as trusted.
    """
    scenario = default_scenario(prepare=_escape_declaration(f"escaped-{name}.txt"))
    run = execute(tmp_path, scenario)
    _assert_refused(run)
    assert "must be inside" in final_diagnostic(run), final_diagnostic(run)


@PROPERTY_SETTINGS
@given(name=_DIRECTORY_NAME)
def test_a_link_that_stays_outside_the_trusted_tree_is_accepted(
    tmp_path: pathlib.Path, name: str
) -> None:
    """Accept a download directory that is a link to a disjoint directory.

    This is the control for the link refusals above: refusing every symbolic
    link would satisfy them and break the ordinary case of a runner whose
    scratch space is itself mounted through one.
    """
    alias = f"via-{name}"
    scenario = default_scenario(
        inputs=Inputs(download_directory=alias),
        prepare=_link_to_empty(alias, f"elsewhere-{name}"),
    )
    _assert_accepted(execute(tmp_path, scenario))
