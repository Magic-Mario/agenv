# AgentEnv — Requirements

## Overview
AgentEnv (`agenv`) is a CLI that gives AI coding agents reproducible,
shareable environments the way `Cargo.lock` gives Rust projects reproducible
dependencies. It lets a team declare, in one file, which agent resources (skills
first) a project depends on, pin those resources to exact commits with
content checksums, and generate the native configuration the agent harness
expects. The MVP targets Claude Code, skills only, Rust, and TOML.

## Goals
- Declare agent dependencies (skills) in a single, human-editable manifest.
- Pin every dependency to an exact git commit + content checksum in a lockfile.
- Reinstall a locked environment byte-for-byte on any machine (reproducibility).
- Generate the native Claude Code layout (`.claude/skills/<name>/`) from the lock.
- Provide a fast, Rust-native CLI named `agenv`.
- Scope isolation to **dependency + configuration** only; never sandboxing.

## Non-goals
- Multi-harness support (Codex, Gemini, OpenCode, Cursor) — deferred.
- MCP servers, plugins, subagents, policies, hooks — deferred (post-MVP).
- A central registry / skill distribution service — deferred (Project 2).
- Filesystem, process, network, or credential isolation — out of scope.
- `agenv run` (spawning an agent inside an isolated env) — out of scope.
- Non-git sources (http tarballs, local paths) — deferred.
- Windows symlinks / junctions for materialization — copy, not link.

## Requirements
Each requirement is numbered, testable, and phrased as observable behavior.

- **R1** — When `agenv init` runs in a directory, it SHALL create `agenv.toml`
  containing `name`, `harness = "claude-code"`, and an empty `[skills]` table,
  plus a `.gitignore` entry for the local store/cache.
- **R2** — When `agenv add <name> --source <url> --ref <ref>` runs, it SHALL
  upsert a `[skills]` entry mapping `name` to `{ source, ref }` and SHALL reject
  an empty name, a non-git `source`, or an empty `ref`.
- **R3** — When `agenv install` runs without a lock, it SHALL resolve each
  skill's `ref` to a concrete commit, compute a content checksum of that commit's
  tree, and write both to `agenv.lock`.
- **R4** — When `agenv install` runs with a lock present and a skill's manifest
  entry unchanged, it SHALL reuse the locked `commit` + `checksum` (frozen
  install) without re-resolving.
- **R5** — When a locked skill's fetched content fails checksum verification, the
  command SHALL abort with a non-zero exit and leave the previous installed state
  untouched (no partial writes).
- **R6** — When `agenv install` succeeds, it SHALL materialize each skill into
  `.claude/skills/<name>/` by copying from the content-addressed store.
- **R7** — The lockfile SHALL be deterministic: `version = 1`, skills sorted by
  name, and reproducible checksums (stable ordering, `.git` excluded).
- **R8** — When `agenv status` runs, it SHALL report drift between manifest and
  lock (added / removed / ref-changed / checksum-mismatch) and SHALL exit
  non-zero if drift exists.
- **R9** — When `agenv update [<name>]` runs, it SHALL re-resolve the named
  skill (or all) to its current `ref`'s latest commit and update the lock.
- **R10** — The tool SHALL only ever manage dependency resolution and
  configuration generation; it SHALL NOT introduce process, filesystem, network,
  or credential isolation.
- **R11** — The manifest and lock SHALL be strict TOML, and `ref`/`source` SHALL
  be quoted strings so version-like values (e.g. `v1.2.0`) are never corrupted.

## Acceptance criteria
- R1: `agenv init` in an empty dir produces `agenv.toml` with the exact schema
  and no error.
- R2: adding the same name twice updates its entry; an empty `ref` exits non-zero
  with a message.
- R3: installing against a local git fixture yields a lock whose `commit` and
  `checksum` match the fixture HEAD.
- R4: a second install with an unchanged manifest performs no network fetch.
- R5: corrupting a stored skill tree, then installing, exits non-zero and the
  prior `.claude/skills/` contents remain.
- R6: `.claude/skills/<name>/SKILL.md` exists after a successful install.
- R7: two fresh installs from scratch produce byte-identical `agenv.lock`.
- R8: after `agenv add` (without install), `agenv status` reports the new skill
  and exits non-zero.
- R9: moving a fixture tag to a new commit, then `agenv update`, updates the lock
  commit.
- R10: code review of the diff shows no sandboxing/process/credential code paths.
- R11: a manifest containing `ref = 1.2.0` fails TOML parsing (value must be a
  string), proving no silent float coercion.
