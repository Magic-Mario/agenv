# AgentEnv (`agenv`)

Reproducible, shareable agent environments — the `Cargo.lock` for AI coding
agent skills. Declare which skills a project depends on, pin them to an exact
git commit + content checksum, and generate the native layout for each harness.

## Prerequisites

- Rust (stable) — to build.
- `git` on your `PATH` — `agenv` shells out to the git CLI for all resolution
  and extraction.

## Build / run / test

```sh
cargo build --release          # binary at target/release/agenv
cargo test                     # unit + e2e (uses a local git fixture)
cargo clippy --all-targets     # lint
cargo fmt --check              # formatting
```

## Usage

```sh
agenv init                                     # create agenv.toml + .gitignore entry
agenv add \
  --source https://github.com/mattpocock/skills/blob/main/skills/engineering/grill-with-docs/SKILL.md
                                                # infers name/ref/path from the URL
agenv add my-skill \
  --source https://github.com/acme/skills \
  --ref v1.2.0 --path skills/rust-reviewer     # or be explicit
agenv add my-skill --source ./skills/my-skill  # or add a local (non-git) skill
agenv install                                  # resolve, lock, materialize
agenv sync                                     # adopt unknown skills, then install
agenv sync --prune                             # install + delete skills not in the manifest
agenv status                                   # report drift (non-zero if dirty)
agenv update [my-skill]                        # re-resolve to the latest commit
```

`add` infers the skill `name`, `ref`, and subdirectory `path` from GitHub
`blob`/`tree` URLs. For a plain repo URL it defaults `ref` to the default branch
and `name` to the repo name; a `path` defaults the name to its last component.
Pass the name as the first argument (or `--ref`/`--path`) to override, and it
prompts for a name when it cannot infer one.

`install` materializes each skill into `<harness>/skills/<name>/` from a
content-addressed store. `harness` is a list; each entry maps to a base
directory: `claude-code` → `.claude`, `opencode` → `.opencode`. A skill with a
`path` is copied from that subdirectory only. The store lives at
`~/.local/share/agenv/store/` (override with the `AGENV_STORE` env var).

A skill may instead live locally in `skills/<name>/` (no git repo). Declare it
with a relative `source = "./skills/<name>"`; `agenv add my-skill --source
./skills/my-skill` does this automatically. Local skills are copied directly
into the harness directories and are never recorded in `agenv.lock`.

`sync` adopts skills found on disk but missing from the manifest — in the
harness directories and in `skills/` — and then installs. A skill is recovered
from its git source when known (via the lock); a skill already under `skills/`
is declared directly; otherwise it is copied into `skills/<name>/` from a
harness dir and declared with a relative `source = "./skills/<name>"`.
`sync --prune` instead deletes any `<harness>/skills/<name>/` not in the
manifest (it never touches `skills/`).

## Files

- `agenv.toml` — hand-authored manifest (`name`, `harness` (a list of harness
  names), `[skills]` with `source`, `ref`, and optional `path`).
- `agenv.lock` — machine-generated lock (`version`, `[[skills]]` with `commit`
  and `checksum`), sorted by name. Commit both to your repo.
- `skills/` — optional local skill sources (git-tracked), one directory per
  skill, referenced by a relative `source`.

## Behavior notes

- `ref` may be a tag, branch, or full commit sha.
- `source` may be a git URL (`https://`, `ssh://`, `git@…`, `…git`), a local git
  repository path (a directory with its own `.git`), or a local directory
  (relative path, no git) that is copied directly and left out of the lock.
- `path` points at a skill subdirectory inside a multi-skill repo; omit it when
  the repo root is the skill.
- A locked install is frozen: `install` reuses the locked commit when `source`,
  `ref`, and `path` are unchanged (no network). Only `update` re-resolves.
- On any checksum mismatch the command aborts without touching the installed
  tree.

## Scope

Skills only, Claude Code + OpenCode, no sandboxing/process/network/credential
isolation. MCP servers, plugins, other harnesses, and a registry are deferred.
