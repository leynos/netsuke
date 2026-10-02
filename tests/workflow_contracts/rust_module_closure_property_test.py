"""Generated tests for the Rust module reader and its closure rules.

The example-based tests in ``rust_module_closure_test.py`` pin one synthetic
crate. This module generates small file-backed crates instead and checks the
closure against an oracle that never reads Rust text: each crate is rendered
from an explicit edge list, so the expected reached set is a plain graph
reachability over that list, not a second parse of the source.

The generated crates are flat (every module is a child of the root) and avoid
`impl` headers, because the closure documents both as over-approximating
where the oracle would have to guess; the rules that over-approximate are
pinned by the example-based tests. Within that space the closure must be
exact: every required dependency reached, and nothing test-only.

Run via ``make test-workflow-contracts``.
"""

import dataclasses
import tempfile
from pathlib import Path

import pytest
from hypothesis import given, settings
from hypothesis import strategies as st
from rust_module_closure import kani_seeds, reachable_files
from rust_module_graph import ModuleGraphError, read_crate

#: How a module refers to another: a rooted path, a final-segment `use`, a
#: `use` group, or a call of the macro the other module defines.
EDGE_KINDS = ("rooted", "final", "group", "macro")
MAX_MODULES = 6
MAX_EDGES = 10
MAX_INCLUDES = 3
#: Generated crates are tiny, so a modest budget explores them well and keeps
#: each example, which writes files, cheap.
PROPERTY = settings(max_examples=60, deadline=None)


@dataclasses.dataclass(frozen=True, slots=True)
class Spec:
    """A generated crate: which modules exist and how they refer to each other."""

    count: int
    is_test_only: tuple[bool, ...]
    is_seed: tuple[bool, ...]
    edges: frozenset[tuple[int, str, int]]
    includes: frozenset[tuple[int, int]]


@st.composite
def specs(draw: st.DrawFn) -> Spec:
    """Draw a small crate specification."""
    count = draw(st.integers(min_value=1, max_value=MAX_MODULES))
    index = st.integers(min_value=0, max_value=count - 1)
    flags = st.tuples(*[st.booleans()] * count)
    edge = st.tuples(index, st.sampled_from(EDGE_KINDS), index)
    include = st.tuples(index, st.integers(min_value=0, max_value=MAX_INCLUDES))
    return Spec(
        count,
        draw(flags),
        draw(flags),
        frozenset(draw(st.lists(edge, max_size=MAX_EDGES))),
        frozenset(draw(st.lists(include, max_size=MAX_INCLUDES))),
    )


def _module_lines(spec: Spec, index: int) -> list[str]:
    """Return the source lines of module ``index``, one construct per line."""
    lines = []
    if spec.is_seed[index]:
        lines.append("#[kani::proof] fn proof() {}")
    if not spec.is_test_only[index]:
        lines.append(f"macro_rules! mac{index} {{ () => {{}}; }}")
    for n, (source, kind, target) in enumerate(sorted(spec.edges)):
        if source != index:
            continue
        lines.append(
            {
                "rooted": f"fn r{n}() {{ crate::m{target}::f(); }}",
                "final": f"use crate::m{target};",
                "group": f"use crate::{{m{target}, m{target}}};",
                "macro": f"fn c{n}() {{ mac{target}!(); }}",
            }[kind]
        )
    lines.extend(
        f'const D{n}: &str = include_str!("../data/d{data}.txt");'
        for n, (source, data) in enumerate(sorted(spec.includes))
        if source == index
    )
    return lines


def render(spec: Spec, *, declaration_order: list[int], rotation: int = 0) -> dict:
    """Return the crate's files, declaring modules and lines in a chosen order."""
    files = {
        "src/lib.rs": "\n".join(
            ("#[cfg(test)]\n" if spec.is_test_only[i] else "") + f"mod m{i};"
            for i in declaration_order
        )
        + "\n"
    }
    for index in range(spec.count):
        lines = _module_lines(spec, index)
        shift = rotation % len(lines) if lines else 0
        files[f"src/m{index}.rs"] = "\n".join(lines[shift:] + lines[:shift]) + "\n"
    return files


def oracle(spec: Spec) -> set[str]:
    """Return the files the closure must reach, by graph reachability alone."""
    compiled = {i for i in range(spec.count) if not spec.is_test_only[i]}
    reached = {i for i in compiled if spec.is_seed[i]}
    pending = set(reached)
    while pending:
        pending = {
            target
            for source, _, target in spec.edges
            if source in pending and target in compiled
        } - reached
        reached |= pending
    # The root declares every module, so it joins whenever any module does.
    files = {f"src/m{i}.rs" for i in reached} | ({"src/lib.rs"} if reached else set())
    return files | {f"data/d{data}.txt" for i, data in spec.includes if i in reached}


def closure(
    spec: Spec, *, declaration_order: list[int] | None = None, rotation: int = 0
) -> set[str]:
    """Write the crate, run the production closure, and return relative paths."""
    order = list(range(spec.count)) if declaration_order is None else declaration_order
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory).resolve()
        for relative, text in render(
            spec, declaration_order=order, rotation=rotation
        ).items():
            (root / relative).parent.mkdir(parents=True, exist_ok=True)
            (root / relative).write_text(text, encoding="utf-8")
        crate = read_crate(root / "src" / "lib.rs")
        return {
            path.relative_to(root).as_posix()
            for path in reachable_files(crate, kani_seeds(crate))
        }


@PROPERTY
@given(spec=specs())
def test_closure_equals_graph_reachability(spec: Spec) -> None:
    """Reach exactly the modules the edge list reaches from the compiled seeds.

    Every required dependency must be present, and a test-only module, or
    anything only a test-only module refers to, must be absent.
    """
    assert closure(spec) == oracle(spec), f"closure differs from oracle for {spec}"


@PROPERTY
@given(spec=specs(), data=st.data())
def test_closure_is_stable_under_declaration_and_line_order(
    spec: Spec, data: st.DataObject
) -> None:
    """Return the same set however the modules and their lines are ordered."""
    order = data.draw(st.permutations(range(spec.count)))
    rotation = data.draw(st.integers(min_value=0, max_value=MAX_EDGES))
    reordered = closure(spec, declaration_order=order, rotation=rotation)
    assert reordered == closure(spec), f"order {order}/{rotation} changed {spec}"


@PROPERTY
@given(spec=specs(), data=st.data())
def test_adding_a_reference_or_seed_never_shrinks_the_closure(
    spec: Spec, data: st.DataObject
) -> None:
    """Keep every reached file when a supported reference or a seed is added."""
    index = st.integers(min_value=0, max_value=spec.count - 1)
    extra = data.draw(st.tuples(index, st.sampled_from(EDGE_KINDS), index))
    seed = data.draw(index)
    seeds = tuple(is_seed or i == seed for i, is_seed in enumerate(spec.is_seed))
    grown = dataclasses.replace(spec, edges=spec.edges | {extra}, is_seed=seeds)
    assert closure(spec) <= closure(grown), f"{grown} lost a file of {spec}"


@PROPERTY
@given(spec=specs(), data=st.data())
def test_a_test_only_module_never_seeds_or_joins_the_closure(
    spec: Spec, data: st.DataObject
) -> None:
    """Ignore a test-only module even when it carries the Kani marker."""
    index = data.draw(st.integers(min_value=0, max_value=spec.count - 1))
    flagged = tuple(flag or i == index for i, flag in enumerate(spec.is_test_only))
    marked = tuple(flag or i == index for i, flag in enumerate(spec.is_seed))
    hidden = dataclasses.replace(spec, is_test_only=flagged, is_seed=marked)
    assert f"src/m{index}.rs" not in closure(hidden), "a test-only module was reached"


def _refusal(spec: Spec, path: str, appended: str) -> None:
    """Append ``appended`` to one generated file and run the closure."""
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory).resolve()
        files = render(spec, declaration_order=list(range(spec.count)))
        files[path] += appended
        for relative, text in files.items():
            (root / relative).parent.mkdir(parents=True, exist_ok=True)
            (root / relative).write_text(text, encoding="utf-8")
        crate = read_crate(root / "src" / "lib.rs")
        reachable_files(crate, kani_seeds(crate))


@PROPERTY
@given(spec=specs(), name=st.from_regex(r"[a-z]{1,8}", fullmatch=True))
def test_a_module_declared_inside_a_block_is_refused(spec: Spec, name: str) -> None:
    """Raise rather than guess where an inline module's `mod x;` resolves."""
    with pytest.raises(ModuleGraphError, match="nested in a block"):
        _refusal(spec, "src/m0.rs", f"\nmod block_{name} {{ mod hidden_{name}; }}\n")


@PROPERTY
@given(spec=specs(), name=st.from_regex(r"[a-z]{1,8}", fullmatch=True))
def test_a_declared_module_without_a_file_is_refused(spec: Spec, name: str) -> None:
    """Raise when a `mod x;` names a file that does not exist."""
    with pytest.raises(ModuleGraphError, match="has no file"):
        _refusal(spec, "src/lib.rs", f"mod absent_{name};\n")


@PROPERTY
@given(spec=specs())
def test_a_non_literal_include_in_a_reached_file_is_refused(spec: Spec) -> None:
    """Raise when a reached file's include names no literal path."""
    marked = dataclasses.replace(
        spec,
        is_test_only=(False, *spec.is_test_only[1:]),
        is_seed=(True, *spec.is_seed[1:]),
    )
    with pytest.raises(ModuleGraphError, match="without a literal path"):
        _refusal(marked, "src/m0.rs", '\nconst X: &str = include_str!(env!("X"));\n')


@PROPERTY
@given(
    kinds=st.lists(st.sampled_from(EDGE_KINDS), min_size=1, max_size=MAX_MODULES - 1)
)
def test_a_chain_of_any_reference_forms_is_followed_to_its_end(
    kinds: list[str],
) -> None:
    """Follow a seed through a chain where each link uses one reference form.

    A general crate rarely draws a module whose only way in is one particular
    form, so this family makes each link the sole route to the next module and
    fails if any reference form stops being followed.
    """
    count = len(kinds) + 1
    chain = Spec(
        count,
        (False,) * count,
        (True, *(False,) * len(kinds)),
        frozenset((i, kind, i + 1) for i, kind in enumerate(kinds)),
        frozenset(),
    )
    expected = {"src/lib.rs", *(f"src/m{i}.rs" for i in range(count))}
    assert closure(chain) == oracle(chain) == expected, f"chain {kinds} broke"
