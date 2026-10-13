# Sandbox target + bundle artifact — Tasks

## Phase 1 — Declaration

- [ ] **T1** — `src/manifest.rs`: add `SandboxSpec` and `NetworkSpec` and
  `Manifest.sandbox: Option<SandboxSpec>` with the documented defaults; serialize
  it in `Manifest::save` and omit the section when absent. Round-trip unit test
  for a manifest with and without `[sandbox]` (R1).
  - depends on: none
- [ ] **T2** — `src/manifest.rs`: `validate_sandbox` — required `image`; `timeout`
  as duration or positive integer seconds; positive `resources` values;
  `skills_root` a safe relative path and `workdir` absolute; `default_action` in
  `{allow, deny}`; `allow`/`deny` entries as dotted domain, wildcard domain, IP,
  or CIDR (the prefix bound depends on the address family); every
  `[sandbox.env]` value an indirection (R2, R3, R4). Wire it into the shared
  validation path in `src/commands/mod.rs` so `install`, `sync` and `bundle`
  reject the same inputs — `sandbox request` calls it directly instead (see T4).
  - depends on: T1
  - note: unit tests for each rejection, including `MY_KEY = "oops"`.

## Phase 2 — Emission

- [ ] **T3** — `Cargo.toml`: add `serde_json`. `src/commands/sandbox.rs` (new):
  `CreateSandboxRequest` plus `NetworkPolicy`/`NetworkRule`/`CredentialProxy`
  emit structs; duration → seconds normalization; deny rules before allow rules;
  omit undeclared keys; `run(out)` writes to stdout or a file (R5, R6).
  - depends on: T2
- [ ] **T4** — `src/main.rs`: `Sandbox { Request { out } }` subcommand. With no
  `[sandbox]`, exit non-zero naming the section and write nothing to stdout; call
  `validate_sandbox` directly, because this path does not go through
  `resolve_manifest` (R8).
  - depends on: T3
- [ ] **T5** — `Cargo.toml`: add `tar`, `flate2`. `src/bundle.rs` (new):
  deterministic `tar.gz` writer — sorted entries, mtime `0`, uid/gid `0`, empty
  uname/gname, ustar framing, no pax extras, gzip header `mtime(0)` and a fixed
  compression level (R10).
  - depends on: none
- [ ] **T6** — `src/commands/bundle.rs` (new): reuse `commands::resolve_manifest`
  for resolution and checksum verification; archive `agenv.toml`, `agenv.lock`
  and every tree at `<skills_root>/<name>/` excluding `.git`; abort without
  writing on a checksum mismatch or an unresolvable skill; print
  `sha256:` + path, `--json` for `{path, sha256, skills[]}` (R9, R11, R12, R13).
  - depends on: T5, T1

## Phase 3 — Tests & docs

- [ ] **T7** — `tests/e2e.rs`: bundle contents (`agenv.toml`, `agenv.lock`,
  `<skills_root>/<name>/SKILL.md`, no `.git`); determinism — two runs, identical
  `sha256`; corrupting a stored tree aborts and leaves no artifact; a manifest
  skill with no lock entry aborts naming it (R9, R10, R11, R12). Use the existing
  local fixture repo and `AGENV_STORE`.
  - depends on: T6
- [ ] **T8** — `tests/e2e.rs` + recorded manual check: `sandbox request` is
  byte-identical across runs and contains no host path, user name, or timestamp;
  missing `[sandbox]` exits non-zero with empty stdout; the emitted document
  validates against `specs/sandbox-lifecycle.yml` and
  `osb sandbox create -f <file>` accepts it — record the command and its output
  as the task's evidence (R6, R7, R8).
  - depends on: T4
- [ ] **T9** — `README.md`: document `[sandbox]` (every field and its default),
  `agenv bundle` (determinism and what the artifact contains),
  `agenv sandbox request`, and the refs-only rule for `[sandbox.env]`. Extend the
  `Scope` section to say the tool emits a sandbox description but never creates
  one (R14 is worth stating to readers, not just to reviewers).
  - depends on: T4, T6

## Cross-spec notes

- **Known follow-up, not fixed here (`documentation` found it).** `Lock::load`
  (`src/lock.rs:33`) still wraps `toml::from_str` in `.with_context`, so a
  malformed `agenv.lock` prints the parser's rendered source line — and a lock's
  `source` URL can embed a credential, which is exactly why `store::redact`
  exists. `Manifest::load` was tightened in this spec (R4's guarantee); the rule
  should move into one shared helper that both callers use. It is unrelated to
  `[sandbox]` and is a `src/` behaviour change, so it belongs in its own task.
- `agenv.schema.json` (owned by `specs/schema-validation`) needs the `[sandbox]`
  table, the same way it needed the flat `[skills]` format.
- The MCP/hooks/agents spec will extend the bundle's contents and owns harness
  version handling (`opencode` V1 `.opencode/agent/` vs V2 `.opencode/agents/`).
- A driver that actually creates a sandbox — `osb` invocation or SDK, volume vs
  push transport, Credential Vault bindings — is a separate spec. This one stops
  at "artifact + document", which is exactly what that spec consumes.
