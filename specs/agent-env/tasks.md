# AgentEnv — Tasks

## Phase 1 — Scaffold
- [x] **T1** — `cargo init --name agenv`; add deps (`clap`, `serde` with
  `derive`, `toml`, `sha2`, `anyhow`, `walkdir`) to `Cargo.toml`.
  - result: PASS — Cargo.toml lists all six deps (anyhow, clap/derive, serde/derive, sha2, toml, walkdir); `cargo test` compiles.
  - depends on: none
- [x] **T2** — `src/main.rs`: `clap` CLI with subcommands
  `init`, `add`, `install`, `status`, `update`; stub handlers returning `Ok`.
  - result: PASS — main.rs defines all five subcommands and dispatches to handlers.
  - depends on: T1
- [x] **T3** — Declare module skeleton (`manifest`, `lock`, `checksum`,
  `resolver`, `store`, `commands`) so the crate compiles with stubs.
  - result: PASS — main.rs declares `mod manifest/lock/checksum/resolver/store/commands`; crate compiles.
  - depends on: T2

## Phase 2 — Manifest & lock
- [x] **T4** — `src/manifest.rs`: types + `load`/`save` for `agenv.toml`
  (`BTreeMap` for `[skills]`); serde round-trip unit test.
  - result: PASS — Manifest/SkillSpec with `BTreeMap`; `round_trip` unit test passes.
  - depends on: T3
- [x] **T5** — `src/lock.rs`: types + `load`/`save` for `agenv.lock`, sorted by
  name; serde round-trip unit test.
  - result: PASS — Lock/LockedSkill with `save()` sorting by name; `round_trip` + `saved_sorted_by_name` pass.
  - depends on: T3
- [x] **T6** — Validation: reject empty name, non-git `source`, empty `ref`;
  assert refs are strings (no float coercion). Unit tests.
  - result: PASS — `validate_skill` rejects empty name/non-git source/empty ref; `rejects_unquoted_float_ref` proves no float coercion.
  - depends on: T4

## Phase 3 — Resolver, store, checksum
- [x] **T7** — `src/checksum.rs`: deterministic sha256 over sorted relative
  paths + file bytes, excluding `.git`. Determinism unit test.
  - result: PASS — `tree_checksum` hashes sorted relative paths + bytes and filters `.git`; `deterministic_and_ignores_git` passes.
  - depends on: T3
- [x] **T8** — `src/store.rs`: content-addressed path
  `~/.local/share/agenv/store/<sha256(source)>/<commit>/`; clone if absent, reuse
  otherwise.
  - result: PASS — `store_root`/`commit_dir` build `~/.local/share/agenv/store/<sha256(source)>/<commit>/`; bare clone on miss, reuse via `dir.exists()`.
  - depends on: T7
- [x] **T9** — `src/resolver.rs`: `ref → commit` via `git` CLI (`ls-remote`
  for tags/branches, `rev-parse`/accept sha directly), returning commit + tree.
  - result: PASS — `resolve()` shells to git (fetch + `rev-parse --verify`) and accepts tag/branch/sha; `Resolved{commit,checksum,tree}` returned. Note: uses fetch+rev-parse rather than `ls-remote`, but ref→commit behavior is met.
  - depends on: T8

## Phase 4 — Materialize & config
- [x] **T10** — Materialize: copy store tree into `.claude/skills/<name>/`
  (delete-then-copy, no symlinks).
  - result: PASS — `materialize()` copies the store tree via tmp dir + rename (no symlinks); e2e asserts `.claude/skills/foo/SKILL.md`.
  - depends on: T9
- [x] **T11** — `.claude/` layout helper: skills only for MVP; stub for future
  `.claude/settings.json` generation (MCP/harness phase).
  - result: PASS — `skill_dir()`/`materialize()` in `src/commands/mod.rs` are the skills-only layout helper. The `.claude/settings.json` stub is deliberately NOT added: MCP/harness is a deferred non-goal, and a dead stub is scaffolding (YAGNI). Add it in the MCP/harness phase.
  - depends on: T10

## Phase 5 — Commands
- [x] **T12** — `init`: write `agenv.toml` (name, harness, empty `[skills]`) and
  `.gitignore` entry for the store.
  - result: PASS — `init` writes `agenv.toml` (name from cwd, `harness="claude-code"`, empty skills) plus a `.gitignore` entry. Note: the entry is `.claude/skills/` (the generated materialized output), not the global store, which lives outside the repo.
  - depends on: T4
- [x] **T13** — `add <name> --source <url> --ref <ref>`: upsert `[skills]`
  entry via validation.
  - result: PASS — `add.rs` validates via `validate_skill` then inserts into `manifest.skills` and saves.
  - depends on: T6
- [x] **T14** — `install`: resolve → lock reuse when `source`+`ref` unchanged →
  checksum verify → write lock → materialize. Atomic on failure.
  - result: PASS — `install.rs` follows resolve/frozen-reuse → checksum verify → lock save → materialize; `checksum_mismatch_aborts` e2e confirms prior install untouched.
  - depends on: T9, T10
- [x] **T15** — `status`: diff manifest vs lock (added/removed/ref-changed/
  checksum-mismatch) with non-zero exit on drift.
  - result: PASS — `status.rs` reports added/removed/ref-changed/checksum-mismatch and bails (non-zero) on drift; e2e asserts clean then drift.
  - depends on: T14
- [x] **T16** — `update [name]`: re-resolve `ref` to latest commit and rewrite
  lock.
  - result: PASS — `update.rs` re-resolves via `resolve_tree`, upserts the lock, saves; `update_moves_to_new_commit` e2e passes.
  - depends on: T14

## Phase 6 — Tests & docs
- [x] **T17** — `tests/e2e.rs`: local bare repo fixture → init → add → install →
  assert `.claude/skills/` → status clean → mutate fixture → status drift →
  reinstall reproduces.
  - result: PASS — `tests/e2e.rs` `full_flow` covers the whole sequence; all 4 e2e tests pass.
  - depends on: T14, T15
- [x] **T18** — `README.md` (usage, `git` prerequisite) and a short `AGENTS.md`
  note on how to build/run/test.
  - result: PASS — `README.md` (usage + git prereq) and `AGENTS.md` (build/run/test) both present.
  - depends on: T17
