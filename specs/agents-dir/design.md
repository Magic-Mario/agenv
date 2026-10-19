# Agents dir — Design

## What changes
`agenv` had a *set* of harnesses, each mapping a name to a base directory:

| harness | base |
| --- | --- |
| `claude-code` | `.claude` |
| `opencode` | `.opencode` |

and materialized every skill into `<base>/skills/<name>/`. This change collapses
the set to a single constant, the portable Agent Skills directory, and drops the
name→base map along with everything that existed only to choose among entries.

```rust
/// The portable Agent Skills directory every current coding agent reads, and
/// the one layout `agenv` materializes into: `.agents/skills/<name>/SKILL.md`.
pub const AGENTS_DIR: &str = ".agents";
```

`.agents/skills` is the path a sandbox's `skills_root` already defaults to, so
the local layout, the bundle, and the sandbox now agree.

## Removals
- `Manifest.harness` (the field), and the `harness` array written by
  `Manifest::save`.
- `HARNESSES`, `supported_harnesses()`, `validate_harness()`.
- `harness_base()` and `src/commands/harness.rs` (the whole file).
- The `harness add` subcommand and the `init --harness` option.
- Every `for harness in &manifest.harness { … }` loop: they become a single
  target directory.

## Kept, reshaped
- `checked_skills_dir()` keeps its two `ensure_not_symlink` guards (on `.agents`
  and on `.agents/skills`) but takes no harness; `skill_dir(name)`,
  `materialize(tree, name)` and `materialize_all(items, verb)` lose their harness
  parameter.
- `SKILLS_DIR = "skills"` stays: it names the git-tracked *local skill sources*,
  which are unrelated to the generated `.agents/skills/` output.
- `sync`'s adoption order is unchanged (lock → `skills/` → vendor from the target
  dir); only the target dir is now `.agents/skills` instead of each harness's.

## Legacy manifests
`Manifest` no longer has the field, and serde ignores unknown TOML keys, so a
manifest with `harness = [...]` still deserializes. `Manifest::save` builds the
document from scratch, so the key disappears the first time the file is
rewritten (any `add`/`sync`). No migration step, no warning, no failure — the
field was never load-bearing once the target is fixed.

## CLI
`agenv init` loses `--harness` and the `harness add` subcommand is deleted, so
old invocations now fail as unknown-argument/unknown-subcommand (clap's default),
which is the desired "this no longer exists" signal (R6).

## Verification
- Unit (`src/manifest.rs`): a legacy `harness` key loads and is dropped on save;
  the existing round-trip tests run without the field.
- E2e (`tests/e2e.rs`): `init` creates `.agents/skills/` and its gitignore entry
  and no `.claude`/`.opencode`; install/status/sync all target `.agents/skills/`;
  adoption vendors a stray `.agents/skills/<name>`; `--prune` deletes undeclared
  ones and leaves `skills/` alone; the retired `harness`/`--harness` invocations
  exit non-zero.

## Known ceilings (not introduced by this change)
- `agenv status` does not call `validate_skill` on manifest entries, so a
  hand-crafted skill name such as `"../../x"` resolves outside `.agents/skills`.
  The command is read-only; `install`/`sync`/`update` do validate. A follow-up
  could validate names in `status` for consistency.
- In `sync`'s vendor fallback, `src.is_dir()` cannot be false by construction
  (the name was discovered as a directory under `.agents/skills/`), but the
  check is kept because it preserves the previous "not found on disk" error
  path instead of silently vendoring an empty tree.

