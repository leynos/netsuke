"""The target derivation, and the premise it rests on.

`cargo_test_targets` models Cargo's auto-discovery rather than reading
`cargo metadata`. Two things have to hold for that to be sound: no target may
be declared by hand, and the derivation must agree with this tree's layout.
Both are asserted here rather than assumed.

Run via ``make test-workflow-contracts``.
"""

from cargo_test_targets import TESTS_DIR, declared_test_targets, target_sources


def test_no_test_target_is_declared_by_hand() -> None:
    """The premise the target derivation rests on.

    `target_sources` models Cargo's auto-discovery: `tests/<name>.rs` and
    `tests/<name>/main.rs` are targets named `<name>`, and everything else
    under `tests/` is a module of one of them. An explicit `[[test]]` section
    can give a target a name unrelated to its path, and the derivation would
    then be describing files rather than targets while still looking right.

    If this ever fails the derivation needs extending to read the manifests,
    not relaxing.
    """
    declared = declared_test_targets()
    assert not declared, (
        f"this workspace declares {len(declared)} `[[test]]` target(s), so "
        f"Cargo no longer names every integration test after its path and the "
        f"derivation in `target_sources` must read the manifests instead"
    )


def test_a_module_under_a_target_is_not_a_target_of_its_own() -> None:
    """Targets are derived, and the derivation is driven by this tree.

    `tests/` here holds both shapes and a great many module files. Globbing
    every `.rs` beneath it reported each module as a target of its own, named
    after its own file, so `tests/<name>/main.rs` read as `main` and would have
    needed an override called `main` that names nothing nextest runs.
    """
    targets = target_sources()
    assert "main" not in targets, (
        "a `tests/<name>/main.rs` target is called `<name>`, not `main`"
    )
    for name, sources in targets.items():
        assert all(source.exists() for source in sources), f"{name} names a gap"
    directories = {
        entry.name
        for entry in TESTS_DIR.iterdir()
        if entry.is_dir() and (entry / "main.rs").exists()
    }
    assert directories <= set(targets), (
        f"every directory holding a `main.rs` is a target: {sorted(directories)}"
    )
    files = {path.stem for path in TESTS_DIR.glob("*.rs")}
    assert files <= set(targets), "every `tests/<name>.rs` is a target"
    assert set(targets) == files | directories, (
        f"nothing else under tests/ is a target; got {sorted(set(targets))}"
    )
