# Local skills + sync adopt — Design

## Source model

A skill's `source` is one of two kinds, decided by a new predicate
`is_local_source(source)`:

| kind | detection | resolution | locked? |
|---|---|---|---|
| git | `is_git_source(source)` (existing) | store + `git checkout` (existing) | yes |
| local | `Path::new(source).is_dir() && !is_git_source(source)` | the directory itself | **no** |

`is_local_source` lives in `manifest.rs` next to `is_git_source`. A local source
must be a **relative** path (no `..`, no `:`), enforced by reusing `is_safe_path`.

## Manifest schema

- `SkillSpec.ref` gains `#[serde(default)]` so a local skill may omit `ref` and
  deserializes to `""` (R8). Git skills keep failing validation on an empty `ref`,
  so this does not weaken the git path.
- A local skill serializes as:
  ```toml
  [skills.my-skill]
  source = "./skills/my-skill"
  ref = ""            # or the user's value if they set one
  ```

## Local skills directory

Skills that are not in a repo live under a project-level `skills/` directory
(const `SKILLS_DIR = "skills"` in `manifest.rs`, git-tracked). The manifest
records them as `source = "./skills/<name>"` — relative and portable, never an
absolute PC path. The materialized output still goes to `<base>/skills/<name>/`
(unchanged). `skills/` is NOT gitignored; `init` keeps ignoring only
`<base>/skills/`.

## Validation (`validate_skill`)

```
if is_local_source(source):
    require is_safe_path(source)          # relative, no '..'
    skip git-source check, skip empty-ref check, ignore path
else:
    existing behavior (git source + non-empty ref + safe subpath)
```

## `resolve_manifest` (shared by install/sync)

For each manifest skill:

- **local**: push `(name, PathBuf::from(source))` to `resolved`, remove any stale
  lock entry (`Lock::remove`), and skip the lock/store pipeline entirely.
- **git**: unchanged (frozen-lock shortcut or `resolve_tree`).

`materialize`/`copy_tree` already copy an arbitrary directory, so local skills
need no new copy path.

## `sync`: adopt then reconcile

New flow:

1. Load manifest + lock.
2. **Adopt** (`fn adopt(manifest, lock)`): collect skill names from (a) every
   declared harness's `<base>/skills/` and (b) `skills/`; for each name not in
   the manifest, recover its source in priority order:
   1. in `lock` → `source`/`ref`/`path` from the lock entry (git skill);
   2. exists under `skills/<name>/` → `source = "./skills/<name>"`, `ref = ""`;
   3. only in a harness dir → copy that dir into `skills/<name>/` (vendor), then
      `source = "./skills/<name>"`, `ref = ""`.
   Save the manifest if anything was added.
3. `resolve_manifest` → lock + resolved; save lock; `materialize_all`.
4. **`--prune` only** (`Sync { prune: bool }`): run the old `prune_stale` — delete
   every `<base>/skills/<name>/` not in the manifest. `--prune` never touches
   `skills/` (that is user-owned source).

Adopt runs before resolve so newly adopted skills materialize in the same run.
`--prune` and adopt are mutually exclusive modes: without the flag `sync` adopts;
with it, `sync` deletes instead.

Symlink safety: vendoring into `skills/<name>/` applies the same guard as
`checked_skills_dir` (refuse if `skills/` or a component is a symlink).

## `add`: auto-detect source kind (R9)

Replace the hard `!is_git_source` bail: accept either a git source (existing path)
or a local directory (`is_local_source`). For local:

- `ref` = `--ref` if given, else `""` (no `default_branch` resolution — that is
  git-only);
- `path` = `None`;
- name inferred from the last path component (already works).

Absolute local paths are rejected by `validate_skill` (`is_safe_path`), so the
recorded source is always relative. Vendoring/copying into `skills/` is `sync`'s
job, not `add`'s.

## `status` and `update`

- `status`: local skills have no lock entry, so skip the lock comparison — report
  `missing` only when a harness dir is absent; never `added`/checksum drift.
- `update`: a local skill has no ref to bump — skip it (no-op) with a note, never
  run `resolve_tree` on a local source.

## Error handling

| Boundary | Behavior |
|---|---|
| local source is absolute or has `..` | error from `validate_skill` |
| `add` source is neither git nor an existing dir | error naming both options |
| `update` targets a local skill | skipped, not an error |

## Files touched

- `src/manifest.rs` — `#[serde(default)]` on `ref`; `is_local_source`; `SKILLS_DIR`;
  local branch in `validate_skill`.
- `src/lock.rs` — `Lock::remove`.
- `src/commands/mod.rs` — local-aware `resolve_manifest`; symlink guard for
  vendoring.
- `src/commands/sync.rs` — `adopt`, `--prune` flag, new flow.
- `src/commands/add.rs` — auto-detect local vs git.
- `src/commands/status.rs` — local-skill branch (no lock comparison).
- `src/commands/update.rs` — skip local skills.
- `src/main.rs` — `Sync { prune }`.
- `tests/e2e.rs` — local install, adopt (harness + skills dir), `--prune`.
