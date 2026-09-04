# AgentEnv — Design

## Architecture
Single Rust binary `agenv`, split into small modules with no shared state:

- `main.rs` — entry point, `clap` argument parsing, dispatch to commands.
- `manifest.rs` — parse/serialize `agenv.toml`.
- `lock.rs` — parse/serialize `agenv.lock` (sorted, deterministic).
- `checksum.rs` — deterministic tree checksum.
- `resolver.rs` — `ref → commit` resolution via the `git` CLI.
- `store.rs` — content-addressed immutable cache.
- `commands/` — `init`, `add`, `install`, `status`, `update`.

Dependencies (each justified):
- `clap` — subcommands + flags (standard Rust choice; not hand-rolled).
- `serde` + `toml` — the whole reason TOML was chosen; first-class in Rust.
- `sha2` — content checksums.
- `anyhow` — error propagation without boilerplate.
- `walkdir` — recursive tree traversal for checksums.
- No libgit2: the tool shells out to the `git` CLI (documented prerequisite) to
  avoid vendoring C and build complexity.

## Data flow
1. `agenv add --source <URL>` → infer `name`/`ref`/`path` from the URL (or prompt
   for a name), then upsert `[skills].<name>` → serialize back (stable key order).
2. `agenv install` → read manifest → for each skill: if lock entry exists and
   `source`+`ref`+`path` unchanged → reuse locked commit; else resolve `ref →
   commit`. Compute checksum over the skill's subdirectory (the whole tree when
   `path` is absent) → write lock → copy to `.claude/skills/<name>/`.
3. `agenv status` → diff manifest vs lock → print drift table → exit code.
4. `agenv update [name]` → re-resolve `ref` to latest commit → rewrite lock.

## Data model
Manifest (`agenv.toml`):
```toml
name    = "payments-backend"
harness = "claude-code"

[skills]
rust-reviewer   = { source = "https://github.com/acme/skills", ref = "v1.2.0" }
grill-with-docs = { source = "https://github.com/mattpocock/skills", ref = "main", path = "skills/engineering/grill-with-docs" }
```
Rust: `{ name: String, harness: String, skills: BTreeMap<String, SkillSpec> }`
where `SkillSpec = { source: String, ref: String, path: Option<String> }`. A map
(not array) enforces unique names, which must be unique in `.claude/skills/`
anyway. `path` selects a subdirectory inside a multi-skill repo (monorepo); when
absent, the repo root is the skill.

Lock (`agenv.lock`), machine-generated:
```toml
version = 1

[[skills]]
name     = "rust-reviewer"
source   = "https://github.com/acme/skills"
ref      = "v1.2.0"
commit   = "f3c2a1b9..."
checksum = "sha256:..."
```
Rust: `{ version: u32, skills: Vec<LockedSkill> }`, sorted by `name`. `LockedSkill`
carries an optional `path` (serialized only when set) so a path change is a
detected drift. Lock uses an array (`[[skills]]`) because it is ordered machine
output; the manifest uses a map because it is hand-authored intent.

## Error handling
| Boundary | Behavior |
|---|---|
| TOML parse failure | message + line/column, non-zero exit |
| Invalid `source`/`ref` | validation error before any network |
| `git` not installed | "git not found" hint, non-zero exit |
| clone/fetch failure (network/auth) | error, no lock written |
| `ref` not found on remote | error naming the skill and ref |
| checksum mismatch | abort, non-zero, prior install untouched |
| `.claude/skills/<name>` collision | error naming the conflict |

All writes are ordered so the lock is written only after all checksums pass, and
materialization happens only after the lock is written (no partial state).

## Testing strategy
- Unit: manifest/lock serde round-trip; checksum determinism (order + `.git`
  excluded); `ref → commit` resolution against a local bare repo fixture.
- One focused integration check (`tests/e2e.rs`): init → add (local fixture) →
  install → assert `.claude/skills/` → status clean → mutate fixture → status
  drift → reinstall reproduces.
- Manual: run the above by hand on a real public skill repo to confirm git-CLI
  behavior against GitHub.

## Decisions / tradeoffs
- **TOML over YAML** — version strings like `1.2.0` are parse errors in TOML
  (forced to quote) instead of silently becoming floats; also Rust-native.
- **`git` CLI over `git2`** — no vendored C, no build headaches; `git` is a
  documented prerequisite. Upgrade to `git2` only if performance demands it.
- **`source` in the manifest** — there is no registry, so the git URL is the
  locator and must be hand-authored; the lock holds only the *resolved* state
  (commit + checksum). Mirrors Cargo's `{ git = ..., tag = ... }`.
- **`[skills]` map over `[[skills]]` array** — idiomatic (Cargo
  `[dependencies]`), terser, and enforces uniqueness. Hooks (future) will stay an
  array because their order matters.
- **Content-addressed global store** (`~/.local/share/agenv/store/<sha256(source)>/<commit>/`)
  over project-local — dedups across projects and is immutable.
- **Copy over symlink** for materialization — portable (Windows symlinks need
  privileges) and avoids git/symlink edge cases.
- **Frozen-vs-resolve split** — install reuses the lock when `source`+`ref` are
  unchanged; `update` is the only command that re-resolves. Separates "reproduce
  exactly" from "move forward".
- **Path-scoped skills** — a skill may live in a subdirectory of a multi-skill
  repo (e.g. `mattpocock/skills`). `add` infers the `path` from a GitHub
  blob/tree URL; checksum and materialization then operate on that subdirectory
  only, never the whole repo.

## Risks
- Skills have no standard version field today, so `ref` (tag/sha) is the only
  version source; a skill author may not tag releases. Mitigation: `ref` accepts
  any sha/branch/tag.
- Shelling to `git` makes `agenv` dependent on git's presence/version; acceptable
  for MVP, documented.
- Checksum semantics (which files count, symlinks) need to match between install
  and verify or every install will false-fail; the unit test on determinism is the
  guard.
