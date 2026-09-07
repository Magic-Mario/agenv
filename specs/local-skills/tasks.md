# Local skills + sync adopt — Tasks

- [x] **T1** — `src/manifest.rs`: add `#[serde(default)]` to `SkillSpec.ref`
  (R8); add `is_local_source(source)` (existing non-git directory); add
  `SKILLS_DIR = "skills"` const; local branch in `validate_skill` (relative
  `is_safe_path`, skip git-source/empty-ref/path checks) (R1, R7). Update unit
  tests.
  - result: PASS — `#[serde(rename = "ref", default)]` (manifest.rs:20), `SKILLS_DIR` const (manifest.rs:78), `is_local_source` (manifest.rs:163-165), and the local branch returning early after `is_safe_path` (manifest.rs:52-60); unit tests cover detection/safe-path/skip-git (manifest.rs:260-314), all 18 unit tests pass.
  - depends on: none
- [x] **T2** — `src/lock.rs`: add `Lock::remove(name)` (drop a skill's lock
  entry; used when a skill is now local).
  - result: PASS — `Lock::remove` implemented via `skills.retain(|s| s.name != name)` (lock.rs:54-56).
  - depends on: none
- [x] **T3** — `src/commands/mod.rs`: make `resolve_manifest` local-aware — for
  a local skill push `(name, source_dir)` to `resolved`, remove any stale lock
  entry, skip the store pipeline (R1, R2). Add a symlink-safe helper for writing
  under `skills/` (vendoring).
  - result: PASS — `resolve_manifest` local branch calls `lock.remove(name)` then pushes `(name, checked_local_source(...))` and `continue`s past the store pipeline (mod.rs:149-152); symlink-safe vendoring helpers `checked_local_skills_dir` (mod.rs:74-78), `checked_local_source` (mod.rs:82-90), and `vendor_skill` (mod.rs:93-102).
  - depends on: T1, T2
- [x] **T4** — `src/commands/sync.rs`: add `adopt` (collect names from harness
  dirs + `skills/`; recover source in order lock → `skills/` → vendor-from-harness,
  never an absolute path) and wire it before resolve (R4, R5). Add `--prune`
  (delete non-manifest `<base>/skills/` dirs, never `skills/`) (R6).
  - result: PASS — `adopt` (sync.rs:40-109) collects names from harness dirs + `skills/` and recovers source in order lock → `skills/` → `vendor_skill`, always writing relative `local_source`; wired before `resolve_manifest` (sync.rs:17-22); `--prune` runs `prune_stale` over harness dirs only, never `skills/` (sync.rs:121-145). e2e `sync_adopts_harness_skill_vendors`/`sync_adopts_local_skills_dir`/`sync_prune_deletes_undeclared` all pass.
  - depends on: T3
- [x] **T5** — `src/commands/add.rs`: auto-detect local vs git — accept a local
  directory (R9); for local set `ref = --ref or ""` (no `default_branch`) and
  `path = None`.
  - result: PASS — `local = is_local_source(...)` gates the git-only branch and bails only when neither git nor an existing dir (add.rs:30-36); local `path = None` (add.rs:38-42) and `ref = reference or ""` skipping `default_branch` (add.rs:56-66).
  - depends on: T1
- [x] **T6** — `src/commands/status.rs`: local-skill branch — skip lock
  comparison, report `missing` only when a harness dir is absent.
  - result: PASS — `is_local_source` branch reports `missing` only when `skill_dir` is absent and `continue`s past the lock comparison (status.rs:21-30).
  - depends on: T1
- [x] **T7** — `src/commands/update.rs`: skip local skills (no ref to bump),
  never run `resolve_tree` on a local source.
  - result: PASS — `is_local_source` branch prints a note and `continue`s before `resolve_tree` (update.rs:27-30).
  - depends on: T1
- [x] **T8** — `src/main.rs`: `Sync { prune: bool }` flag.
  - result: PASS — `Sync { #[arg(long)] prune: bool }` declared (main.rs:34-38) and dispatched to `commands::sync::run(prune)` (main.rs:61).
  - depends on: T4
- [x] **T9** — `tests/e2e.rs`: local install (no lock entry); `sync` adopts a
  harness-dir skill (vendors into `skills/`, relative source); `sync` adopts a
  `skills/`-only skill; `sync --prune` deletes an undeclared skill. Keep existing
  tests green.
  - result: PASS — `local_skill_install_no_lock_entry` (e2e.rs:383-406), `sync_adopts_harness_skill_vendors` (e2e.rs:409-436), `sync_adopts_local_skills_dir` (e2e.rs:439-464), and `sync_prune_deletes_undeclared` (e2e.rs:467-489); `cargo test` = 28 passed (18 unit + 10 e2e).
  - depends on: T4
- [x] **T10** — `README.md`: document local skills (`skills/`, `source =
  "./skills/<name>"`), `sync` adopt, and `sync --prune`.
  - result: PASS — README documents local skills + `source = "./skills/<name>"` (README.md:52-55), `sync` adopt + vendoring (README.md:57-60), `sync --prune` (README.md:61-62), and the `skills/` directory (README.md:70-71).
  - depends on: T4
