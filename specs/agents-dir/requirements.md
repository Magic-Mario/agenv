# Agents dir — Requirements

## Overview
Remove the harness-selection feature from `agenv`. Instead of asking which coding
agent to target and materializing a copy per harness (`.claude/skills/`,
`.opencode/skills/`), `agenv` always materializes skills into the single portable
Agent Skills layout, `.agents/skills/<name>/SKILL.md`, which current coding
agents read directly.

## Goals
- One materialization target for every project: `.agents/skills/`.
- No harness to choose, declare, or validate — in the manifest or on the CLI.
- Manifests written before this change keep working (the old `harness` key is
  simply ignored) and stop carrying it once rewritten.

## Non-goals
- Migrating existing `.claude/` / `.opencode/` content into `.agents/` — the user
  moves files if they want to; nothing is read from the old dirs anymore.
- Changing what a *sandbox*'s `skills_root` defaults to (that already is
  `.agents/skills`).
- Any new layout knob or config; `.agents` is a constant, not a setting.

## Requirements
Each requirement is numbered, testable, and phrased as observable behavior.

- **R1** — `agenv.toml` SHALL NOT define a `harness` field. A manifest that still
  contains a `harness` key SHALL load successfully (the key is ignored) and SHALL
  be rewritten without it on the next save.
- **R2** — `agenv init` SHALL create `agenv.toml` (`name` + empty `[skills]`),
  create the directory `.agents/skills/`, and append `.agents/skills/` to
  `.gitignore`. It SHALL take no harness option.
- **R3** — `agenv install`, `agenv sync`, and `agenv update` SHALL materialize
  each declared skill into `.agents/skills/<name>/` from the store (or from its
  local source), exactly as before but for the one target.
- **R4** — `agenv status` SHALL check `.agents/skills/<name>/` (existence and
  checksum) and SHALL exit non-zero on drift.
- **R5** — `agenv sync` SHALL adopt skills found under `.agents/skills/` and
  under `skills/`, and SHALL no longer read `.claude/` or `.opencode/`.
  `agenv sync --prune` SHALL delete any `.agents/skills/<name>/` not in the
  manifest and SHALL never touch `skills/`.
- **R6** — The `harness` subcommand and the `--harness` option SHALL not exist;
  invoking either SHALL fail as an unknown argument or subcommand.
- **R7** — Materialization SHALL keep its existing guarantees (delete-then-copy
  via a temp dir, no symlinks followed) and its symlinked-path guards; only the
  target path changes.

## Acceptance criteria
- R1: a manifest containing `harness = ["claude-code"]` loads and, after any
  command that saves it, no longer contains `harness`.
- R2: `agenv init` in an empty dir creates `.agents/skills/` and a `.gitignore`
  containing `.agents/skills/`, and creates neither `.claude/` nor `.opencode/`.
- R3: after `agenv install`, `.agents/skills/<name>/SKILL.md` exists.
- R4: mutating a materialized `.agents/skills/<name>/SKILL.md` makes `agenv
  status` exit non-zero; a clean tree exits zero.
- R5: a skill dir placed under `.agents/skills/` and not in the manifest is
  adopted by `agenv sync`; `agenv sync --prune` deletes it, and a dir under
  `skills/` is left alone.
- R6: `agenv harness add opencode` and `agenv init --harness opencode` both exit
  non-zero.
- R7: `install` aborts on a store checksum mismatch leaving the previous
  `.agents/skills/` contents untouched.
