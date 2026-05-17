# Architectural decision record (ADR) 003: Agent-consistent human-first CLI

## Status

Accepted.

Accepted: 2026-05-17. Netsuke will make its command-line interface human-first
in presentation and agent-consistent in structure before the first public
release.

## Date

2026-05-17.

## Context and problem statement

Netsuke is pre-0.1.0, so the project can still remove inconsistent command,
option, and output vocabulary without carrying compatibility aliases. The CLI
overhaul needs a stable doctrine that keeps the local human experience clear
while making automation reliable enough for agents, CI systems, and other tools.

The active execution plan in
[`docs/execplans/netsuke-cli-overhaul.md`](execplans/netsuke-cli-overhaul.md)
records the governing decisions. This ADR turns those decisions into a durable
architecture record so later implementation work does not need to rediscover
the same trade-offs.

## Decision drivers

- Preserve Netsuke's approachable human interface while removing ambiguous
  command grammar.
- Provide one structured result contract that automation can parse without
  mixing human prose and machine data.
- Keep destructive and consequential operations explicit.
- Avoid duplicating generic command-contract machinery that belongs in
  OrthoConfig.
- Preserve completed roadmap work unless a relevance audit proves it obsolete.
- Enforce vocabulary and documentation consistency mechanically in CI.

## Options considered

### Option A: keep the friendly CLI and add aliases for agents

This would keep current human-facing affordances and add compatibility aliases
or agent-only variants where needed.

It was rejected because aliases would preserve the inconsistencies the
pre-0.1.0 overhaul is intended to remove. A separate agent vocabulary would
also create two public contracts to document, test, and support.

### Option B: make the CLI agent-first

This would optimize command names, output, and error shapes for automation,
even where that makes ordinary terminal usage less pleasant.

It was rejected because Netsuke remains a human-operated build tool. Automation
support must improve predictability without making the normal CLI feel like a
wire protocol.

### Option C: use a human-first CLI with agent-consistent structure

This keeps the CLI readable for people while making names, output modes, exit
codes, mutation rules, and diagnostics stable enough for automation.

It is accepted because it satisfies both audiences with one coherent command
surface.

## Decision outcome

Netsuke adopts a human-first, agent-consistent CLI contract.

- The bare `netsuke` invocation remains equivalent to `netsuke build`.
- Legacy spellings and inconsistent vocabulary are removed rather than
  documented as aliases.
- `--json` is the only structured result mode.
- JSON mode owns both successful results and failure diagnostics.
- JSON mode must not mix subprocess output into stdout.
- Destructive operations require `--force`.
- Consequential operations provide `--dry-run`.
- Planned product surfaces include `context`, `skill-path`, `runs`, `profile`,
  delivery, and feedback commands.
- Vocabulary consistency is enforced mechanically in CI.
- Existing roadmap tasks are preserved unless a relevance audit shows the
  redesigned interface makes them obsolete.

Shared command-contract features depend on the active OrthoConfig roadmap,
especially tasks `5.2.3`, `6.1`, `6.2`, `6.3`, `7.1`, `7.2`, `8.1`, `9.1`,
`9.2`, and `9.3`. Netsuke records hard dependencies where work must wait, soft
dependencies where temporary local adaptation is acceptable, and Netsuke-owned
build semantics where OrthoConfig should not reach.

## Known risks and limitations

- Human-first wording can drift back into inconsistent command grammar if CI
  does not check vocabulary.
- Depending on OrthoConfig may delay generic command-contract work that Netsuke
  needs for the CLI overhaul.
- Removing legacy spellings before 0.1.0 is acceptable, but any downstream
  users of unreleased snapshots may need to adjust scripts.
- JSON mode must define ownership of subprocess output carefully so build
  output remains useful without corrupting structured stdout.

## Related documents

- [`docs/execplans/netsuke-cli-overhaul.md`](execplans/netsuke-cli-overhaul.md)
- [`docs/netsuke-design.md`](netsuke-design.md)
- [`docs/roadmap.md`](roadmap.md)
- [`docs/archive/roadmap-completed-foundations.md`](archive/roadmap-completed-foundations.md)
