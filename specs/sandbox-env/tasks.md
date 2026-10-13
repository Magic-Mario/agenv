# Sandbox target + bundle artifact — Tasks

## Phase 1 — Declaration

- [x] **T1** — `src/manifest.rs`: add `SandboxSpec` and `NetworkSpec` and
  `Manifest.sandbox: Option<SandboxSpec>` with the documented defaults; serialize
  it in `Manifest::save` and omit the section when absent. Round-trip unit test
  for a manifest with and without `[sandbox]` (R1).
  - depends on: none
  - result: PASS — `SandboxSpec`/`NetworkSpec` at `src/manifest.rs:38,78`; `Manifest.sandbox` is `Option<SandboxSpec>` with `skip_serializing_if = "Option::is_none"` (`:15`); `save` emits the table at `:172` and omits it when `None`. Both required tests exist: `sandbox_round_trip` (`:775`) and `sandbox_absent_is_omitted` (`:815`) assert full `Manifest` equality across save→load (behaviour, not formatting — the header `.contains("[sandbox]")` assert is additional); `sandbox_defaults_are_applied` (`:831`) pins the documented defaults. Reproduced in `/tmp/opencode/sbx-roundtrip`: a CLI load→save→load via `agenv add` kept the `[sandbox]` block byte-identical and re-loaded via `install` (exit 0); a no-sandbox manifest stayed clean. `cargo test` 26 unit (4 sandbox) + 15 e2e pass.
- [x] **T2** — `src/manifest.rs`: `validate_sandbox` — required `image`; `timeout`
  as duration or positive integer seconds; positive `resources` values;
  `skills_root` a safe relative path and `workdir` absolute; `default_action` in
  `{allow, deny}`; `allow`/`deny` entries as dotted domain, wildcard domain, IP,
  or CIDR (the prefix bound depends on the address family); every
  `[sandbox.env]` value an indirection (R2, R3, R4). Wire it into the shared
  validation path in `src/commands/mod.rs` so `install`, `sync` and `bundle`
  reject the same inputs — `sandbox request` calls it directly instead (see T4).
  - depends on: T1
  - note: unit tests for each rejection, including `MY_KEY = "oops"`.
  - result: PASS — `validate_sandbox` (`src/manifest.rs:333`) enforces every R2/R3/R4 rule; wired into the shared path via `resolve_manifest` at `src/commands/mod.rs:164` (used by `install`/`sync`), with `update` calling it directly at `src/commands/update.rs:13`. Reproduced at HEAD with `target/release/agenv` in `/tmp/opencode/sbx-verify`: R2 (no `image` → exit 1, `missing field \`image\``), each R3 rejection (timeout `soon`, `skills_root ../etc`, `workdir /../etc`, `default_action maybe`, `allow ["not a domain"]`, `allow ["*.1.2.3.4"]`, `cpu "1e3"`, env key with a space) exits 1 naming the field, and R4 rejects `MY_KEY = "oops"` / `MY_KEY = "sk-ant-live-…"` naming the key and the `env:REAL_VAR` form without echoing the value, while `MY_KEY = "env:THE_REAL_VAR"` is accepted (exit 0); an unquoted pasted secret fails at parse with position only (`src/manifest.rs:122`). Note on the wiring text: `bundle` and `sandbox request` do not exist yet (T6/T4) — `bundle` will inherit the shared path by reusing `resolve_manifest`, so this clause is anticipatory, not drift; `update` plus `install`/`sync` all reject the same inputs. R5–R13 are T3–T6, out of reach at this HEAD. `cargo fmt --check` clean, `cargo clippy --all-targets` 0 warnings, 26 + 15 tests pass.

## Phase 2 — Emission

- [ ] **T3** — `Cargo.toml`: add `serde_json`. `src/commands/sandbox.rs` (new):
  `CreateSandboxRequest` plus `NetworkPolicy`/`NetworkRule`/`CredentialProxy`
  emit structs; duration → seconds normalization; deny rules before allow rules;
  always emit the fail-closed network policy, omit empty collections and a
  disabled credential proxy; `run(out)` writes to stdout or a file (R5, R6).
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
  validates against OpenSandbox's published `specs/sandbox-lifecycle.yml`
  (upstream, not vendored in this repository) and
  `osb sandbox create -f <file>` accepts it — record the command and its output
  as the task's evidence (R6, R7, R8). The `entrypoint` question is answered:
  a document declaring neither `entrypoint` nor `resourceLimits` was accepted, so
  the schema's prose about them is not enforced — the check records that
  measurement instead of re-litigating it. It must also record the two server
  prerequisites it cannot check: `credentialProxy.enabled` requires the server to
  run `[egress] mode = "dns+nft"` (the default local config does not), and
  `timeout` must sit under `server.max_sandbox_timeout_seconds` (86400 by
  default, `30h` measured rejected, and the schema publishes no `maximum`).
  - depends on: T4
- [ ] **T9** — `README.md`: document `[sandbox]` (every field and its default),
  `agenv bundle` (determinism and what the artifact contains),
  `agenv sandbox request`, and the refs-only rule for `[sandbox.env]`. Extend the
  `Scope` section to say the tool emits a sandbox description but never creates
  one (R14 is worth stating to readers, not just to reviewers), and add the
  operator-facing cautions a reader cannot guess: no field may carry a secret
  (`image` and `resources` values are emitted verbatim, so a credential in one
  would be republished in the document), `credential_proxy` needs the server to
  run `[egress] mode = "dns+nft"`, and `timeout` must sit under the server's
  configured maximum.
  - depends on: T4, T6

## Cross-spec notes

- **Known follow-up, not fixed here — one defect class, two remaining instances.**
  (a) `Lock::load` (`src/lock.rs:33`) still wraps `toml::from_str` in
  `.with_context`, so a malformed `agenv.lock` prints the parser's rendered source
  line, and a lock's `source` URL can embed a credential — which is exactly why
  `store::redact` exists. (b) `validate_sandbox` never echoes an `env` *value*
  (R4 holds) but does interpolate the env *key* and the other fields (`timeout`,
  `resources`, `skills_root`, `workdir`, network targets), so a credential typed
  into one of those positions comes back on stderr. `Manifest::load` was
  tightened in this spec, which is what makes the remaining two visible rather
  than new. The rule should move into one shared helper both callers use; both
  are unrelated to `[sandbox]` correctness and are `src/` behaviour changes, so
  they belong in their own task.
- `agenv.schema.json` (owned by `specs/schema-validation`) needs the `[sandbox]`
  table, the same way it needed the flat `[skills]` format.
- The MCP/hooks/agents spec will extend the bundle's contents and owns harness
  version handling (`opencode` V1 `.opencode/agent/` vs V2 `.opencode/agents/`).
- A driver that actually creates a sandbox — `osb` invocation or SDK, volume vs
  push transport, Credential Vault bindings — is a separate spec. This one stops
  at "artifact + document", which is exactly what that spec consumes.
