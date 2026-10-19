# Agents dir — Tasks

## Phase 1 — Manifest & layout
- [x] **T1** — `src/manifest.rs`: drop the `harness` field and its write in
  `save`; remove `HARNESSES`, `supported_harnesses`, `validate_harness`; add
  `pub const AGENTS_DIR: &str = ".agents"`. Update the unit tests and add one
  proving a legacy `harness` key still loads and is dropped on save.
  - depends on: none
  - result: PASS — `Manifest` has no `harness` field and `save` writes only
    `name` + `[skills]`; the three harness symbols are gone; `AGENTS_DIR` is the
    single target constant; `a_legacy_harness_key_is_ignored` loads
    `harness = ["claude-code"]`, asserts it is dropped on save, and round-trips.
- [x] **T2** — `src/commands/mod.rs`: drop `harness_base` and `pub mod harness`;
  make `checked_skills_dir()`, `skill_dir(name)`, `materialize(tree, name)` and
  `materialize_all(items, verb)` single-target (no harness arg), keeping the
  symlink guards; drop `validate_harness` from `resolve_manifest`.
  - depends on: T1
  - result: PASS — all four helpers are single-target; `checked_skills_dir`
    keeps the `.agents` + `.agents/skills` symlink guards, and `skill_dir` now
    routes through them too; `resolve_manifest` no longer validates a harness.

## Phase 2 — Commands
- [x] **T3** — `src/commands/init.rs` + `src/main.rs`: `init` takes no
  `--harness`, writes no `harness`, creates `.agents/skills/` and
  `.gitignore` entry `.agents/skills/`; remove the `harness` subcommand and
  `HarnessCommand`, and delete `src/commands/harness.rs`.
  - depends on: T2
  - result: PASS — `init` is argument-less and creates `.agents/skills/` with
    the matching gitignore entry; the subcommand, its enum, and the whole
    `harness.rs` file are deleted; e2e `init_creates_agents_dir` and
    `retired_harness_commands_fail` (both retired invocations exit non-zero).
- [x] **T4** — `src/commands/install.rs` + `src/commands/update.rs`: call
  `materialize_all(items, verb)` without a harness list; drop `validate_harness`.
  - depends on: T2
  - result: PASS — both call sites use the single-target signature; `full_flow`
    and `update_moves_to_new_commit` pass.
- [x] **T5** — `src/commands/status.rs`: check the single `.agents/skills/<name>/`
  path; drop `validate_harness` and the per-harness loops/messages.
  - depends on: T2
  - result: PASS — one `skill_dir(name)` path for both local and git skills;
    `full_flow` asserts clean, then drift after tampering, then clean again.
- [x] **T6** — `src/commands/sync.rs`: materialize to the one target; adopt from
  `.agents/skills/` + `skills/` only; `--prune` deletes undeclared dirs under
  `.agents/skills/` and never `skills/`.
  - depends on: T2
  - result: PASS — adoption scans `.agents/skills/` + `skills/`, the vendor
    fallback reads `.agents/skills/`, and `prune_stale` walks only
    `.agents/skills/`. E2e `sync_prunes`, `sync_adopts_agents_skill_vendors`,
    `sync_adopts_local_skills_dir`, and `sync_prune_deletes_undeclared` (which
    now also asserts `skills/keep` survives) pass.

## Phase 3 — Tests & docs
- [x] **T7** — `tests/e2e.rs`: retarget every `.claude/skills` path to
  `.agents/skills`; replace the harness tests with `init` layout assertions
  (`.agents/skills/` created, `.claude`/`.opencode` absent, retired flags exit
  non-zero); retarget adoption/prune fixtures.
  - depends on: T3, T4, T5, T6
  - result: PASS — all materialized paths are `.agents/skills/…`; the two
    harness tests became `init_creates_agents_dir` and
    `retired_harness_commands_fail`; full run is 20 unit + 15 e2e, 0 failed,
    with `cargo clippy --all-targets` 0 warnings and `cargo fmt --check` clean.
    A manual smoke in `/tmp/opencode/agents-smoke` confirmed `init` layout, a
    legacy `harness` key being dropped by `add`, install to
    `.agents/skills/demo/SKILL.md`, and exit 2 for both retired invocations.
- [x] **T8** — `README.md`: describe the single `.agents/skills` target, the
  removed harness field/subcommand, and the manifest schema without `harness`.
  - depends on: T7
  - result: PASS — README describes `.agents/skills/` throughout, the manifest
    schema has no `harness`, and a short paragraph tells users the old harness
    knob is gone while legacy keys still load.
