# Sandbox target + bundle artifact — Requirements

## Overview

Two additions that let `agenv` prepare an environment for an isolated runtime
(OpenSandbox) **without ever touching that runtime**:

1. **`[sandbox]`** — an optional manifest section that declares what a sandbox
   should look like (image, timeout, where skills live, which hosts may be
   reached, whether the Credential Vault proxy is enabled).
2. **`agenv bundle`** — a deterministic archive of the resolved skill trees plus
   `agenv.toml` and `agenv.lock`, for shipping into a sandbox.
3. **`agenv sandbox request`** — renders `[sandbox]` as a CreateSandboxRequest
   JSON document for OpenSandbox's API/CLI.

`agenv` stays a dependency-and-configuration tool: it emits artifacts and
documents, it does not create, connect to, or manage a sandbox.

## Goals

- Declare a sandbox in the same hand-authored file that declares skills.
- Produce one reproducible artifact that carries the pinned environment into a
  sandbox: same inputs → byte-identical archive → meaningful checksum.
- Emit the runtime's own request format, so an operator can pipe it straight
  into `osb sandbox create -f`.
- Make it structurally impossible for `agenv` to leak a credential.

## Non-goals

- Creating, starting, listing, or destroying a sandbox; no `osb` invocation and
  no HTTP client — that is an integration phase, not this one.
- Volume mounts and host binds (`volumes[]` in the wire format).
- Credential Vault bindings (per-sandbox runtime state, created against a live
  sandbox). Only `credentialProxy.enabled` and the network policy are declared.
- MCP servers, hooks, subagents, commands, policies — deferred to their own spec.
- Harness version pinning (`opencode` V1 vs V2 layouts) — see Risks.
- A harness-layout bundle: the archive targets the sandbox layout only;
  `agenv install` remains the way to materialize a local harness.

## Requirements

### Sandbox declaration (`[sandbox]`)

- **R1** — `agenv.toml` SHALL accept an optional `[sandbox]` table with fields:
  `image` (string), `timeout` (string), `skills_root` (string, default
  `.agents/skills`), `workdir` (string, default `/workspace`),
  `credential_proxy` (bool, default false), `resources` (`cpu`/`memory`),
  `[sandbox.env]` (KEY → indirection string `"env:NAME"`), and
  `[sandbox.network]`
  (`default_action`, `allow`, `deny`). A manifest without `[sandbox]` SHALL
  load and behave exactly as today.
- **R2** — `[sandbox].image` SHALL be required whenever `[sandbox]` is present.
  Its absence SHALL fail the command with a non-zero exit naming the field.
- **R3** — Validation SHALL reject, before any work: an empty `image`; a
  `timeout` that is neither a duration (`30m`, `1h`, `45s`) nor a positive
  integer of seconds; a negative, zero, or exponent-form `resources` value; a
  `skills_root` that is not a safe relative path (same rule as a skill `path`);
  a `workdir` that is not absolute or that contains a `..` component; a
  `default_action` other than `allow` or `deny`; a `network.allow`/`deny` entry
  that is not a domain, IP, or CIDR, or a wildcard applied to anything other than
  a domain; and an `[sandbox.env]` key that is not an environment-variable name.
- **R4** — Every value in `[sandbox.env]` SHALL be an indirection of the form
  `env:<NAME>`, where `<NAME>` is an environment-variable name. Any other value
  SHALL be a validation error, and the error SHALL name the key and the accepted
  form **without echoing the value**. `agenv` never holds a secret, and it must
  never print one either: a value that must reach the sandbox is resolved by
  whoever consumes the output, not by `agenv`.

### `agenv sandbox request`

- **R5** — `agenv sandbox request` SHALL write, to stdout, a CreateSandboxRequest
  JSON document (camelCase keys, per OpenSandbox's published
  `specs/sandbox-lifecycle.yml` — upstream, not vendored in this repository)
  describing `image`, `timeout`, `env`, `resourceLimits`, `networkPolicy`, and
  `credentialProxy`; `--out <path>` SHALL write the same document to a file
  instead. The document SHALL state the manifest's *effective* intent rather than
  only its literal keys: `image`, `timeout` and the network policy are always
  emitted, because the manifest's own defaults are part of what it declares and
  because omitting `networkPolicy` leaves the runtime's egress **open** rather
  than denied. Collections with no entries (`env`, `resourceLimits`, `egress`)
  SHALL be omitted, and `credentialProxy` SHALL be emitted only when it is
  enabled — absent and `enabled: false` mean the same thing to the runtime.
- **R6** — The emitted document SHALL be deterministic: byte-identical for
  identical input, with a fixed field order, no host paths, no user identity, no
  timestamps, and no ambient environment values. An `env` indirection SHALL
  appear as its declared reference (e.g. `env:MY_KEY`), never as a resolved
  value.
- **R7** — Field names and value shapes SHALL be validated against
  `specs/sandbox-lifecycle.yml` (upstream), and the document SHALL be accepted by
  `osb sandbox create -f`. `agenv` SHALL NOT guess a field the schema does not
  define.
- **R8** — Running the command without `[sandbox]`, or with an invalid
  `[sandbox]`, SHALL exit non-zero with a message naming the field, and SHALL
  write nothing to stdout.

### `agenv bundle`

- **R9** — `agenv bundle` SHALL write `agenv-bundle.tar.gz` (or `--out <path>`)
  containing: `agenv.toml`, `agenv.lock`, and every skill in the manifest at
  `<skills_root>/<name>/`. A skill's `.git` entry SHALL be excluded, in the same
  way materialization excludes it.
- **R10** — The archive SHALL be deterministic: identical manifest, lock, and
  store content SHALL produce a byte-identical archive. Entry order SHALL be the
  manifest's (name-sorted) order, entry metadata SHALL be normalized (fixed
  mtime, uid/gid 0, no platform-specific extras), and gzip SHALL be written with
  a zeroed header timestamp.
- **R11** — `bundle` SHALL reuse the existing resolution path: each skill's tree
  SHALL come from the resolved store entry and its content SHALL be verified
  against the lock checksum. On a checksum mismatch the command SHALL abort with
  a non-zero exit and write no artifact.
- **R12** — A skill that cannot be resolved (no lock entry, missing store tree,
  or a local source that does not exist) SHALL abort the command with a message
  naming the skill. A bundle SHALL never silently omit a declared skill.
- **R13** — On success the command SHALL print the artifact path and its
  `sha256:` digest; with `--json` it SHALL emit an object with `path`, `sha256`,
  and the sorted list of bundled skill names and their checksums.

### Guardrail

- **R14** — `agenv` SHALL NOT create, connect to, or manage a sandbox, and
  SHALL NOT gain a network client: no new process is spawned beyond the `git`
  the tool already requires. Every value the sandbox needs SHALL be derived from
  `agenv.toml`, `agenv.lock`, and the content-addressed store.

## Acceptance criteria

- R1: a manifest with `[sandbox]` round-trips (load → save → load) unchanged; a
  manifest without it is unaffected.
- R2: `[sandbox]` with no `image` → non-zero exit naming `image`.
- R3: `timeout = "soon"`, `skills_root = "../etc"`, `workdir = "/../etc"`,
  `default_action = "maybe"`, `allow = ["not a domain"]`, `allow = ["*.1.2.3.4"]`,
  `cpu = "1e3"` and an `[sandbox.env]` key containing a space are each rejected.
- R4: `[sandbox.env] MY_KEY = "oops"` and `MY_KEY = "sk-ant-live-…"` are rejected
  by validation with a message that names the key, shows the `env:REAL_VAR` form,
  and does not contain the value; `MY_KEY = "env:THE_REAL_VAR"` is accepted.
- R5: `sandbox request` prints valid JSON; `--out` writes the same bytes to a file.
- R6: two runs produce byte-identical stdout; the output contains no host path,
  no user name, and no timestamp.
- R7: the document validates against OpenSandbox's published
  `specs/sandbox-lifecycle.yml` (upstream; not vendored here), and
  `osb sandbox create -f <file>` accepts it (manual check, recorded in the task).
- R8: `sandbox request` without `[sandbox]` exits non-zero and prints nothing on stdout.
- R9: unpacking the archive yields `agenv.toml`, `agenv.lock`, and
  `<skills_root>/<name>/SKILL.md` for every declared skill, with no `.git` entry.
- R10: two `bundle` runs over the same inputs produce the same `sha256:`.
- R11: corrupting a stored skill tree makes `bundle` exit non-zero and leave no
  artifact behind.
- R12: adding a skill to the manifest without installing it → `bundle` exits
  non-zero naming that skill.
- R13: stdout contains `sha256:` and the path; `--json` parses and lists the skills.
- R14: code review of the diff shows no sandbox client, no HTTP dependency, and
  no new subprocess.
