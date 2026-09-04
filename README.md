# AgentEnv (`agenv`)

Reproducible, shareable agent environments — the `Cargo.lock` for AI coding
agent skills. Declare which skills a project depends on, pin them to an exact
git commit + content checksum, and generate the native Claude Code layout.

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
agenv add rust-reviewer \
  --source https://github.com/acme/skills \
  --ref v1.2.0                                 # declare a skill dependency
agenv install                                  # resolve, lock, materialize
agenv status                                   # report drift (non-zero if dirty)
agenv update [rust-reviewer]                   # re-resolve to the latest commit
```

`install` materializes each skill into `.claude/skills/<name>/` from a
content-addressed store. The store lives at `~/.local/share/agenv/store/`
(override with the `AGENV_STORE` env var).

## Files

- `agenv.toml` — hand-authored manifest (`name`, `harness`, `[skills]`).
- `agenv.lock` — machine-generated lock (`version`, `[[skills]]` with `commit`
  and `checksum`), sorted by name. Commit both to your repo.

## Behavior notes

- `ref` may be a tag, branch, or full commit sha.
- `source` may be a git URL (`https://`, `ssh://`, `git@…`, `…git`) or a local
  git repository path.
- A locked install is frozen: `install` reuses the locked commit when `source`
  and `ref` are unchanged (no network). Only `update` re-resolves.
- On any checksum mismatch the command aborts without touching the installed
  tree.

## Scope

Skills only, Claude Code only, no sandboxing/process/network/credential
isolation. MCP servers, plugins, other harnesses, and a registry are deferred.
