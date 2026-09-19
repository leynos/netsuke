"""Predicates deciding which workflow steps build, and which install first.

``build_standard_wiring_test`` asks three questions of a lane: does it install
the pinned tools, does a given step compile, and in what order do those happen.
Each answer comes from reading a ``run`` script or a Makefile rather than from a
step name, so that renaming a step cannot satisfy the contract and deleting a
command cannot slip past it.

The predicates live here rather than in the test module because they are read
as values — parsed workflow steps and raw Makefile text — and because the test
module would otherwise outgrow the repository's 400-line file limit. Nothing
here touches a file: the caller supplies what was read.

Run via ``make test-workflow-contracts``.
"""

import re

#: The Make target that installs the pinned linker and toolchain.
INSTALL_TARGET = "install-build-tools"

#: The Make target the repository gates on the pinned tools being present. A
#: target that declares it, directly or through a prerequisite, is one whose
#: recipe needs those tools on `PATH` before it runs.
CAPABILITY_TARGET = "check-build-tools"

#: Spellings of `make` a recipe may use. `$(MAKE)` recurses and an absolute
#: path reaches the same binary, so all three must be recognised or the target
#: behind them goes unseen.
MAKE_INVOCATIONS = ("make", "$(MAKE)", "${MAKE}")

#: A Make rule header: a goal, then its prerequisites up to any `##` comment.
#: The goal is restricted to characters that appear in real target names so a
#: variable assignment such as `FOO := bar` cannot be mistaken for one.
RULE_HEADER = re.compile(r"^(?P<goal>[A-Za-z0-9_./$()%-]+):(?!=)(?P<rest>.*)$")

#: The compiler drivers a step may invoke for itself, without a Make target in
#: between.
COMPILER_DRIVERS = ("cargo", "rustc")

#: Arguments that report state rather than build, whichever position they take.
#: A lane's preliminary steps announce their toolchain with `cargo --version`,
#: and counting that as a build would fail the lane for printing its compiler
#: version — as true of the contract's intent as a real regression is.
QUERY_FLAGS = frozenset({"--version", "-V", "--help", "-h", "--list"})

#: Subcommands that report state rather than build. Formatting and cache
#: clearing sit here too: neither produces an artefact needing the pinned
#: linker.
QUERY_SUBCOMMANDS = frozenset({
    "--explain",
    "--print",
    "clean",
    "config",
    "fmt",
    "locate-project",
    "metadata",
    "pkgid",
    "tree",
})


def uses_coverage_action(step: dict[str, object]) -> bool:
    """Return whether ``step`` calls the shared coverage action.

    Returns
    -------
    bool
        True when the step's ``uses`` names the shared coverage action.
    """
    match step.get("uses"):
        case str() as uses:
            return "generate-coverage" in uses
        case _:
            return False


def only_coverage_steps(
    steps: list[dict[str, object]],
) -> list[dict[str, object]]:
    """Return the steps that run the shared coverage action.

    Parameters
    ----------
    steps
        Steps to filter, in declaration order.

    Returns
    -------
    list[dict[str, object]]
        Every step that invokes the coverage action.
    """
    return [step for step in steps if uses_coverage_action(step)]


def rule_prerequisites(makefile: str) -> dict[str, list[str]]:
    """Return each Make goal's prerequisites, order-only ones included.

    Order-only prerequisites count because that is how the file attaches the
    capability check to a file rule: `target/debug/$(APP): | check-build-tools`
    is a real dependency even though Make would not rebuild on it.

    Returns
    -------
    dict[str, list[str]]
        Goal to the prerequisites it declares, in the order the file gives.
    """
    prerequisites: dict[str, list[str]] = {}
    for line in makefile.splitlines():
        if line.startswith(("\t", "#")):
            continue
        match = RULE_HEADER.match(line)
        if match is None:
            continue
        goal = match.group("goal")
        if goal.startswith("."):
            # Special goals such as `.PHONY` list target names rather than
            # prerequisites, and nothing depends on them.
            continue
        declared = match.group("rest").split("##", 1)[0]
        prerequisites.setdefault(goal, []).extend(declared.replace("|", " ").split())
    return prerequisites


def gated_targets(makefile: str) -> frozenset[str]:
    """Return the goals whose recipes need the pinned tools on `PATH`.

    A goal qualifies when it declares the capability check, or when any
    prerequisite of it does — `build` reaches the check through the
    `target/debug/$(APP)` file rule, and `lint` through `lint-clippy`. Walking
    the declared graph rather than matching target names is what keeps this
    correct when a target is renamed.

    Returns
    -------
    frozenset[str]
        Every goal in the file whose recipe the capability check precedes.
    """
    prerequisites = rule_prerequisites(makefile)
    gated: set[str] = set()

    def reachable(goal: str, seen: frozenset[str]) -> bool:
        """Return whether the capability check is reachable from ``goal``.

        ``seen`` carries the goals already walked on this path. A Makefile may
        declare a cycle — mutual prerequisites are legal and simply mean each
        waits for the other — so the walk stops rather than recursing forever.

        Returns
        -------
        bool
            True when some chain of prerequisites leads from ``goal`` to the
            capability check, or when ``goal`` is that check itself.
        """
        if goal in seen:
            return False
        if goal == CAPABILITY_TARGET:
            return True
        next_seen = seen | {goal}
        return any(
            reachable(prerequisite, next_seen)
            for prerequisite in prerequisites.get(goal, [])
        )

    for goal in prerequisites:
        if reachable(goal, frozenset()):
            gated.add(goal)
    return frozenset(gated)


def logical_lines(script: str) -> list[str]:
    """Split a ``run`` script into commands, joining backslash continuations.

    A wrapped command is one command to the shell, so reading it line by line
    would see the binary on one line and its arguments on the next — which is
    exactly the shape the gate recipes use, and would make each half look like
    nothing recognisable.

    Returns
    -------
    list[str]
        One entry per command, continuations already joined.
    """
    logical: list[str] = []
    pending = ""
    for line in script.splitlines():
        stripped = line.strip()
        if stripped.endswith("\\"):
            pending += stripped[:-1] + " "
            continue
        logical.append(pending + stripped)
        pending = ""
    if pending:
        logical.append(pending)
    return logical


def invoked_make_targets(script: str) -> set[str]:
    """Return the Make goals a step's script names.

    A goal is one operand of a command that starts a `make` invocation.

    Options and ``VAR=value`` assignments are dropped, since neither is a goal,
    and every remaining operand is reported. Reporting them all rather than the
    first keeps a command naming two goals out of the single-target test below:
    ``make install-build-tools something-else`` is not the install step, and
    must not be read as one.

    Returns
    -------
    set[str]
        Every operand that is a goal rather than an option or assignment.
    """
    targets: set[str] = set()
    for line in logical_lines(script):
        tokens = line.split()
        if not tokens or tokens[0].rsplit("/", 1)[-1] not in MAKE_INVOCATIONS:
            continue
        operands = [token for token in tokens[1:] if not token.startswith("-")]
        targets.update(token for token in operands if "=" not in token)
    return targets


def installs_build_standard(step: dict[str, object]) -> bool:
    """Return whether ``step`` runs exactly ``make install-build-tools``.

    The whole command must be that one goal. ``install-build-tools-extra`` and
    ``make install-build-tools other-goal`` both invoke something else, and
    accepting either on a substring match is how the step could stop
    installing the standard while the test went on passing.

    Returns
    -------
    bool
        True when the step's ``run`` script names that goal and no other.
    """
    match step.get("run"):
        case str() as script:
            return invoked_make_targets(script) == {INSTALL_TARGET}
        case _:
            return False


def driver_builds(line: str) -> bool:
    """Return whether a shell line drives a compiler for something but a query.

    Returns
    -------
    bool
        True when some driver on the line is asked to do more than report
        state.
    """
    tokens = line.split()
    commands = [
        index
        for index, token in enumerate(tokens)
        if token.rsplit("/", 1)[-1] in COMPILER_DRIVERS
    ]
    for index in commands:
        follows = tokens[index + 1 :]
        if any(token in QUERY_FLAGS for token in follows):
            continue
        # Options before the subcommand are still options (`cargo
        # -Zunstable-options config get ...`), so the driver's first bare word
        # is what it is being asked to do.
        subcommand = next((token for token in follows if not token.startswith("-")), "")
        if subcommand not in QUERY_SUBCOMMANDS:
            return True
    return False


def compiles(step: dict[str, object], gated: frozenset[str]) -> bool:
    """Return whether ``step`` can produce compiled output.

    Three ways in, and all three are needed. The coverage action runs Cargo
    inside a composite action, so its step declares no `run` script at all; a
    step's own script may drive Cargo or rustc directly; and a step may invoke
    a Make target whose recipe is what calls the compiler.

    Parameters
    ----------
    step
        The workflow step to judge.
    gated
        Every Make goal whose recipe the capability check precedes.

    Returns
    -------
    bool
        True when any of the three applies to this step.
    """
    if uses_coverage_action(step):
        return True
    script = step.get("run")
    if not isinstance(script, str):
        return False
    if invoked_make_targets(script) & gated:
        return True
    return any(driver_builds(line) for line in logical_lines(script))


def step_name(step: dict[str, object]) -> str:
    """Return a step's name, or a placeholder for an unnamed step.

    Returns
    -------
    str
        The name, so a failure can cite the step it is about.
    """
    return str(step.get("name") or "<unnamed step>")
