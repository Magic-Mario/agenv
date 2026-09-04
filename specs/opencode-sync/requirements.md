# OpenCode + sync — Requirements

## Overview
Extend `agenv` from Claude-Code-only to multi-harness (Claude Code + OpenCode)
and add a `sync` command that reconciles the generated skill directories with
the manifest — installing what is declared and removing what is not.

## Goals
- Declare one or more target harnesses in `agenv.toml`.
- Materialize skills into each harness's native layout
  (`.claude/skills/<name>/` and/or `.opencode/skills/<name>/`).
- `sync` reconciles the local skill directories to match `agenv.toml` exactly:
  install what is missing, update what changed, remove what was deleted.

## Requirements

- **R1** — `agenv.toml` `harness` SHALL be a list of harness names, non-empty,
  each one of `claude-code` or `opencode`. `agenv init` SHALL write
  `harness = ["claude-code"]`.
- **R2** — Materialization SHALL target `<base>/skills/<name>/` where `base` is
  `.claude` for `claude-code` and `.opencode` for `opencode`.
- **R3** — `agenv install` SHALL materialize every declared skill into every
  declared harness's skill directory.
- **R4** — `agenv sync` SHALL read the manifest, resolve and lock (same pipeline
  as `install`), materialize into every declared harness, and remove any
  materialized skill directory that is not in the manifest.
- **R5** — `agenv sync` SHALL remove a skill from every harness's directory when
  it is deleted from the manifest.
- **R6** — An empty `harness` list, or an unknown harness name, SHALL fail with a
  clear error before any materialization or removal.

## Acceptance criteria
- R1: `agenv init` produces `harness = ["claude-code"]`; a manifest with
  `harness = "claude-code"` (string) fails TOML parsing (no silent coercion).
- R2: after install with `harness = ["opencode"]`, `.opencode/skills/<name>/SKILL.md`
  exists.
- R3: with `harness = ["claude-code", "opencode"]`, install writes both
  `.claude/skills/<name>/` and `.opencode/skills/<name>/`.
- R4/R5: `agenv sync` after removing a skill from the manifest deletes its
  directories under every harness and succeeds; after adding a skill, it
  materializes it.
- R6: `harness = []` and `harness = ["bogus"]` both exit non-zero with a message.
