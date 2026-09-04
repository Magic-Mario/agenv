# OpenCode + sync — Tasks

- [x] **T1** — `src/manifest.rs`: change `harness` to `Vec<String>`; add
  `validate_harness` (non-empty, only `claude-code`/`opencode`). Update unit
  tests.
  - result: PASS — `harness` is `Vec<String>` (manifest.rs:12), `validate_harness` rejects empty and unknown harnesses against `HARNESSES` (manifest.rs:75-88), and unit test `harness_validation` covers it (manifest.rs:183-189).
  - depends on: none
- [x] **T2** — `src/commands/mod.rs`: add `harness_base` (name → base dir);
  make `skill_dir`/`materialize`/`materialize_all` harness-aware; extract
  `resolve_manifest` (resolve + lock shared by install/sync).
  - result: PASS — `harness_base` (mod.rs:26), `skill_dir`/`materialize` take a harness arg (mod.rs:58,64), `materialize_all` iterates harnesses (mod.rs:84-96), and `resolve_manifest` returns `(Lock, Vec<(String, PathBuf)>)` shared by install/sync (mod.rs:100-155).
  - depends on: T1
- [x] **T3** — `src/commands/init.rs`: write `harness = ["claude-code"]`; write
  a `.gitignore` entry per harness base.
  - result: PASS — init.rs writes `harness: vec!["claude-code"]` (init.rs:23) and loops over `manifest.harness` writing `<base>/skills/` gitignore entries via `harness_base` (init.rs:29-31).
  - depends on: T2
- [x] **T4** — `src/commands/install.rs` + `update.rs`: materialize into every
  harness via `resolve_manifest`/`materialize_all`.
  - result: PASS — install.rs uses `resolve_manifest` + `materialize_all(&resolved, &manifest.harness, ...)` (install.rs:9-12); update.rs uses `materialize_all(&updated, &manifest.harness, "updated")` (update.rs:47).
  - depends on: T2
- [x] **T5** — `src/commands/status.rs`: check every harness's materialized
  directory for missing/checksum-mismatch.
  - result: PASS — status.rs loops `for harness in &manifest.harness` checking `skill_dir(harness, name)` existence and `tree_checksum` against the lock (status.rs:34-43).
  - depends on: T2
- [x] **T6** — `src/commands/sync.rs`: new `sync` command = resolve/lock +
  materialize all harnesses + prune stale skill dirs.
  - result: PASS — sync.rs `run` calls `resolve_manifest`, `lock.save`, `materialize_all`, then `prune_stale` which removes non-manifest dirs per harness (sync.rs:10-47).
  - depends on: T2
- [x] **T7** — `src/main.rs`: register `Sync` subcommand.
  - result: PASS — `Command::Sync` variant declared (main.rs:34) and dispatched to `commands::sync::run()` (main.rs:57).
  - depends on: T6
- [x] **T8** — `tests/e2e.rs`: sync flow (add → install to both harnesses →
  remove from manifest → sync prunes). Keep existing tests green.
  - result: PASS — `sync_multi_harness_prunes` test materializes to both `.claude` and `.opencode` and verifies both are pruned after removal (e2e.rs:331-380); full `cargo test` = 20 passed (14 unit + 6 e2e).
  - depends on: T6
- [x] **T9** — `README.md`: document multi-harness and `sync`.
  - result: PASS — README documents `harness` list → base-dir mapping (`claude-code` → `.claude`, `opencode` → `.opencode`) and the `sync` = install + reconcile behavior (README.md:44-52).
  - depends on: T6
