# Local skills + sync adopt — Requirements

## Overview
Skills can currently only come from a git repository. Extend `agenv` so a skill
can also live in a local directory (kept outside `.opencode`/`.claude` and
git-tracked), and change `sync` so it *adopts* skills found on disk instead of
deleting them — with `--prune` as the explicit "delete what is not declared"
escape hatch.

## Goals
- Store skills outside the harness config dirs so they can be committed, pushed,
  and pulled into other projects via git.
- Support a second source kind: a local directory (personal skills not meant for
  a repo). Local skills are not locked.
- `sync` adopts unknown skills (adds them to the manifest); `--prune` restores
  the old delete behavior.
- Cover both adopters: users already running `agenv` (skills already materialized
  in the harness dirs) and new users (skills sitting in the project `skills/`).

## Requirements

- **R1** — A skill MAY declare a **local source**: `source` is a path to a
  directory (no `://`, no `git@`, not ending in `.git`). A local source SHALL
  NOT be recorded in `agenv.lock`.

- **R2** — Materializing a local skill SHALL copy the source directory into every
  declared harness's `<base>/skills/<name>/` (no store, no checkout). Git skills
  keep the existing store + checkout pipeline.

- **R3** — Local skills SHOULD live under a project-level `skills/` directory
  (git-tracked, outside `.opencode`/`.claude`), so they can be shared by
  `source = "<git url>"` + `path = "<skill>"` in another project.

- **R4** — `agenv sync` SHALL adopt: collect skill names from (a) every declared
  harness's `<base>/skills/` and (b) the project `skills/` directory; any name
  not in the manifest is added to it, never deleted.

- **R5** — When adopting, `sync` SHALL recover the source, in this order:
  1. name is in `agenv.lock` → re-add with that git `source`/`ref`/`path`;
  2. else name exists under `skills/` → `source = "./skills/<name>"` (local);
  3. else (found only in a harness dir) → copy its content into `skills/<name>/`
     and record `source = "./skills/<name>"` (a relative path, never absolute).

- **R6** — `agenv sync --prune` SHALL delete every `<base>/skills/<name>/`
  directory whose name is not in the manifest. Without `--prune`, unknown skills
  are adopted, never deleted.

- **R7** — `validate_skill` SHALL accept a local source (an existing directory
  that is not a git repo) and skip the git-source check for it. Manifest
  `source` values for local skills SHALL be relative paths; absolute or
  `..`-escaping paths SHALL be rejected.

- **R8** — A local skill's `ref` SHALL be preserved when provided, and default
  to the empty string `""` when not. Git skills keep the existing `ref`
  requirements (non-empty).

- **R9** — `agenv add` SHALL auto-detect the source kind: a git source follows
  the existing path; an existing local directory is recorded as a local source
  (relative, `ref = ""` unless given) without requiring a `--local` flag.

## Acceptance criteria
- R1/R2: with `source = "./skills/my-skill"` (a directory with `SKILL.md`),
  `agenv install` creates `.opencode/skills/my-skill/SKILL.md` and writes no
  `agenv.lock` entry for it.
- R4/R5: a skill dir dropped into `.opencode/skills/` and not in the manifest is
  added to `agenv.toml` on `sync` with `source = "./skills/<name>"` and its
  content vendored into `skills/<name>/`.
- R4/R5: a skill dir present only under `skills/` and not in the manifest is
  added on `sync` with `source = "./skills/<name>"` and then materialized into
  the harness dirs.
- R5 (lock): removing a git skill from the manifest and running `sync` re-adds it
  with its original git source (recovered from the lock).
- R6: `agenv sync --prune` deletes an undeclared skill dir; `agenv sync` keeps it.
- R7: `source = "/abs/path"` and `source = "../escape"` are rejected.
- R8: a local skill without `ref` serializes `ref = ""`; one with `ref = "x"`
  keeps it.
- R9: `agenv add my-skill ./skills/my-skill` adds a local source; the same
  command with a git URL adds a git source.
