#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = [
#   "polythene-isolate @ git+https://github.com/leynos/polythene.git@a29ccbc3687868f5ae5139d5e3a88ca6bf2e5cc1",
# ]
# ///
"""Run the pinned Polythene CLI for the Linux package validation sandbox.

The shared ``validate-linux-packages`` action launches ``uv run <path>``, so
this file is a PEP 723 script rather than an installed console script: its
inline metadata is what makes ``uv`` provide the pinned Polythene revision.

Polythene must be pinned here because the revision the shared action resolves
for itself predates its local-image probe. That revision treats a bare
``localhost/...`` reference as a registry host, so it fails with a connection
error when the workflow's locally built validation image is the only one
available. The revision pinned above probes ``podman image exists`` first and
pulls only when the image is genuinely absent, which is what lets the images
stay local: the packaging job never pushes them.

This file is deliberately outside the repository's Python lint and typecheck
roots. Those runs would need the pinned dependency installed to resolve the
import below, and the wrapper has no behaviour beyond forwarding ``argv`` to
the CLI that ``validate-linux-packages`` invoked.

The file is named ``polythene_cli.py`` rather than ``polythene.py`` because
Python puts the script's directory first on ``sys.path``. A sibling named
``polythene.py`` shadows the installed package, so ``from polythene.__main__``
resolves to this file and fails with "``polythene`` is not a package".
"""

from polythene.__main__ import main

if __name__ == "__main__":
    main()