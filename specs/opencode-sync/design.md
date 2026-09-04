# OpenCode + sync — Design

## Harness model
`harness` changes from a single string to a `Vec<String>` in `Manifest`. Each
entry maps to a base directory via a single mapping function:

| harness | base |
|---|---|
| `claude-code` | `.claude` |
| `opencode` | `.opencode` |

Skills always land at `<base>/skills/<name>/SKILL.md`, which both harnesses
recognize natively (Claude Code scans `.claude/skills/`; OpenCode scans
`.opencode/skills/`). No other layout differences exist for skills.

## Commands
- `install` — resolve → lock → checksum → materialize into **every** harness.
- `update` — re-resolve → materialize into **every** harness.
- `status` — report drift; checks every harness's materialized directory.
- `sync` — full reconcile: the same resolve/lock/materialize pipeline as
  `install`, then prune any skill directory (in every harness) whose name is not
  in the manifest.

`install` and `sync` share the resolve/lock loop via one helper
(`resolve_manifest`), so the only sync-specific logic is pruning.

## Pruning semantics
For each harness base, list `<base>/skills/` and remove every directory whose
name is not a key in `manifest.skills`. Files (non-directories) are left alone.
The content-addressed store is an immutable cache and is never pruned.

## Error handling
| Boundary | Behavior |
|---|---|
| `harness` empty | error before any work |
| unknown harness name | error naming the harness and the supported set |
| (existing) checksum mismatch / ref not found | unchanged |

## Files touched
- `src/manifest.rs` — `harness: Vec<String>`, `validate_harness`.
- `src/commands/mod.rs` — `harness_base`, harness-aware `skill_dir`/`materialize`/
  `materialize_all`, shared `resolve_manifest`.
- `src/commands/{init,install,update,status}.rs` — harness-aware.
- `src/commands/sync.rs` — new command.
- `src/main.rs` — `Sync` subcommand.
- `tests/e2e.rs` — sync flow test.
