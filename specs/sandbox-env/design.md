# Sandbox target + bundle artifact — Design

## Architecture

Three surfaces, all **emit-only** — nothing here talks to a runtime:

| Surface | File | Responsibility |
|---|---|---|
| Declaration | `src/manifest.rs` | `SandboxSpec` parsing, defaults, validation |
| Artifact | `src/bundle.rs` (new) | deterministic `tar.gz` writer |
| Emission | `src/commands/sandbox.rs` (new) | `[sandbox]` → CreateSandboxRequest JSON |
| Command | `src/commands/bundle.rs` (new) | resolve → verify → archive → checksum |

`bundle` deliberately reuses `commands::resolve_manifest()` — the function
`install` and `sync` already share. It resolves each skill, materializes the
store entry, verifies the content against the lock checksum, and returns
`(Lock, Vec<(name, tree)>)`. No resolution logic is duplicated, and the
checksum guarantee of R11 comes for free. The only difference from `install` is
what happens next: `install` calls `materialize_all`, `bundle` writes an archive
and touches no harness directory.

## Data model

Manifest — the new section, everything else unchanged:

```toml
name    = "life-simulator-mvp"
harness = ["opencode"]

[skills]
grill-with-docs = { source = "https://github.com/mattpocock/skills", ref = "main", path = "skills/engineering/grill-with-docs" }

[sandbox]
image            = "node:22-bookworm-slim"
timeout          = "30m"                  # default: 30m
skills_root      = ".agents/skills"       # default
workdir          = "/workspace"           # default
credential_proxy = true                   # default: false

[sandbox.resources]
cpu    = "1"
memory = "2Gi"

[sandbox.env]
ANTHROPIC_API_KEY = "env:ANTHROPIC_API_KEY"

[sandbox.network]
default_action = "deny"
allow          = ["api.anthropic.com", "registry.npmjs.org"]
deny           = ["*.example.com"]
```

Rust:

```rust
pub struct Manifest {
    pub name: String,
    pub harness: Vec<String>,
    #[serde(default)]
    pub skills: BTreeMap<String, SkillSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<SandboxSpec>,   // new, optional: absent = no behavior change
}

pub struct SandboxSpec {
    pub image: String,
    #[serde(default = "default_timeout")]      pub timeout: String,
    #[serde(default = "default_skills_root")]  pub skills_root: String,
    #[serde(default = "default_workdir")]      pub workdir: String,
    #[serde(default)]                          pub credential_proxy: bool,
    #[serde(default)]                          pub resources: BTreeMap<String, String>,
    #[serde(default)]                          pub env: BTreeMap<String, String>,
    #[serde(default)]                          pub network: NetworkSpec,
}

// `env` values are indirection strings (`"env:REAL_VAR"`), validated by
// `validate_sandbox` — deliberately not a typed struct, because a
// deserialization error is rendered by the TOML parser together with the
// offending source line, which would print the secret this rule protects.

pub struct NetworkSpec {
    #[serde(default = "default_deny")] pub default_action: String,
    #[serde(default)] pub allow: Vec<String>,
    #[serde(default)] pub deny: Vec<String>,
}
```

Emitted document (`agenv sandbox request`), key names and nesting taken from
`specs/sandbox-lifecycle.yml`:

```json
{
  "image": "node:22-bookworm-slim",
  "timeout": 1800,
  "env": { "ANTHROPIC_API_KEY": "env:ANTHROPIC_API_KEY" },
  "resourceLimits": { "cpu": "1", "memory": "2Gi" },
  "networkPolicy": {
    "defaultAction": "deny",
    "egress": [
      { "action": "deny",  "target": "*.example.com" },
      { "action": "allow", "target": "api.anthropic.com" },
      { "action": "allow", "target": "registry.npmjs.org" }
    ]
  },
  "credentialProxy": { "enabled": true }
}
```

`timeout` is normalized from the manifest's duration string to seconds, the unit
the wire format uses. Keys the manifest does not declare are omitted rather than
emitted with a default, so the document never claims more than the author said.

## Data flow

`agenv bundle`:

1. `Manifest::load("agenv.toml")` → `resolve_manifest(&manifest)` — same path as
   `install`, checksum verification included.
2. Archive `agenv.toml`, `agenv.lock`, then each resolved tree at
   `<skills_root>/<name>/…`, in name order, skipping `.git` exactly as
   `copy_tree` does.
3. Print `sha256:` + path, or the `--json` object.

Nothing else is written to the project: no harness directory, no lock rewrite
(the lock is only read; `resolve_manifest` may write it back identical, which is
harmless and already true of `install`).

`agenv sandbox request`:

`Manifest::load` → validate `[sandbox]` → map to the emit structs → serialize.
Pure function of the manifest; no filesystem read beyond the manifest, no store
access.

## Determinism

The archive checksum is only meaningful if the bytes are reproducible, so every
source of incidental variation is normalized:

| Source of variation | Normalization |
|---|---|
| Entry order | manifest order (`BTreeMap`, name-sorted) |
| mtime | fixed `0` for every entry |
| owner/group | uid/gid `0`, `uname`/`gname` empty |
| GNU vs ustar vs pax extras | ustar format, no extra headers |
| gzip header | `mtime(0)`, fixed compression level, no filename |
| Absolute paths | entry names are relative to the archive root |
| `agenv.toml` bytes | included verbatim — the manifest *is* intent, so its formatting is part of the artifact identity |

`[sandbox]` emission is deterministic for the same reason: field order comes from
the Rust struct declaration order (so `serde_json` needs no `preserve_order`
feature), and no value is derived from the host.

## Decisions / tradeoffs

- **`.agents/skills` as the default `skills_root`.** It is the portable Agent
  Skills path, it is what OpenCode discovers (validated end-to-end in the Phase 0
  experiment: a canary skill was found with only that path present), and it is
  what OpenSandbox's own `osb skills --target opencode` writes. Being
  harness-agnostic, one tree covers every harness, so a bundle never needs a
  per-harness copy.
- **Emit-only, with a guardrail requirement (R14).** `specs/agent-env` sets
  "process, filesystem, network, or credential isolation — out of scope" and
  requires that a review shows no sandboxing code paths. That stays true: this
  spec adds a *configuration surface that describes* a foreign runtime and a
  serialization of it. Creating the sandbox belongs to a later integration spec
  and to a different process (the `osb` CLI or an SDK), which keeps the
  dependency graph and the threat surface of `agenv` unchanged.
- **env values are indirections of the form `env:<NAME>` (R4), enforced by
  validation, not by deserialization.** Two reasons, both learned the hard way:
  a scheme-prefixed name cannot be satisfied by accident, so a pasted token is
  rejected rather than accepted as a "reference"; and a deserialization error is
  rendered by the TOML parser *with the offending source line*, so rejecting
  there would print the very secret the rule exists to keep out of logs. A
  literal constant belongs in the image (`ENV` in a Dockerfile), where it is
  reproducible anyway.
- **`image` stays a required `String`, so an absent value fails during
  deserialization** (`missing field \`image\``) while `validate_sandbox` rejects a
  present-but-empty one. Typing it `Option<String>` so every message came from
  one layer would weaken the type for a message-only gain: that error annotates
  the `[sandbox]` header, not a value, so unlike `env` it leaks nothing.
- **`tar` + `flate2` crates instead of shelling out.** The existing pattern is to
  shell out (`git`) rather than vendor C, but reproducible GNU tar flags
  (`--sort=name --mtime=@0 --owner=0`) do not exist in BSD/macOS tar and Windows
  ships no `tar`. Pure-Rust writers keep determinism portable, which is the whole
  point of the artifact.
- **`serde_json` as a new dependency.** Hand-writing JSON is more code and more
  risk than the dependency costs, and struct declaration order already gives the
  fixed field order R6 needs.
- **`deny` rules before `allow` rules.** The wire format evaluates egress rules
  in order and the docs state first-match-wins, so a deny has to be able to
  shadow a broad allow. The manifest keeps the two lists separate for
  readability and the renderer imposes the order.
- **The bundle carries the sandbox layout only.** A harness-layout bundle would
  duplicate what `install` already produces. If a future phase (image baking)
  needs one, it can add a flag then.
- **Validation lives in one place.** `validate_sandbox` is called from the same
  entry point as `validate_harness`/`validate_skill`, so `install`, `sync`,
  `bundle`, and `sandbox request` all reject a broken `[sandbox]` identically.

## Files touched

| File | Change |
|---|---|
| `src/manifest.rs` | `SandboxSpec`/`NetworkSpec`; `Manifest.sandbox`; defaults; `validate_sandbox`; round-trip + validation unit tests |
| `src/bundle.rs` | **new** — deterministic tar.gz writer (ordering, mtime, ownership, gzip header) |
| `src/commands/bundle.rs` | **new** — resolve via `resolve_manifest`, archive, sha256, `--json` |
| `src/commands/sandbox.rs` | **new** — `CreateSandboxRequest` emit structs, `run(out)` |
| `src/commands/mod.rs` | `pub mod bundle; pub mod sandbox;`; call `validate_sandbox` in the shared validation path |
| `src/main.rs` | `Bundle { out, json }`; `Sandbox { Request { out } }` |
| `Cargo.toml` | add `serde_json`, `tar`, `flate2` |
| `tests/e2e.rs` | bundle determinism + contents + abort cases; request determinism + error cases |
| `README.md` | `[sandbox]` reference, `bundle`, `sandbox request`, refs-only rule |
| `specs/schema-validation` | `[sandbox]` in `agenv.schema.json` (cross-spec) |

## Risks

- **Wire shape.** The schema types `image` as `ImageSpec` (`{ uri, auth }`) while
  the CLI takes and returns a bare string (observed in the Phase 0 experiment).
  The emitted value stays the form the CLI accepts, and T8 records an
  `osb sandbox create -f` round-trip as evidence; the field is corrected if that
  check disagrees.
- **`ResourceLimits` declares no properties** in the schema, so `[sandbox.resources]`
  is passed through as a string map rather than validated key by key. Only the
  value shape is checked.
- **Determinism breaks silently.** One mtime, one mode bit, or one pax header is
  enough. The guard is the two-run hash assertion in `tests/e2e.rs`, not a manual
  check.
- **Harness version drift.** Skill discovery paths are version-independent, but
  hooks and agent definitions are not (`opencode` V1 uses `.opencode/agent/`,
  V2 uses `.opencode/agents/`). Out of scope here; it belongs to the MCP/hooks
  spec, and it is the reason `[sandbox]` declares one portable tree instead of a
  per-harness layout.
- **The bundle is only as reproducible as its store.** A missing commit makes
  `bundle` fetch, exactly like `install`. That is documented behavior, not a
  regression, but it means a first bundle on a clean machine needs network.

## Cross-spec dependencies

- `specs/agent-env` — Goals/Non-goals and R10 are unchanged in substance; see the
  emit-only decision above. The "MCP servers, plugins, subagents, policies,
  hooks — deferred" non-goal is untouched.
- `specs/schema-validation` owns `agenv.schema.json` and must gain the `[sandbox]`
  table (mirrors how it had to gain the flat `[skills]` format).
- The MCP/hooks/agents spec will extend what the bundle carries and owns
  harness-version handling; `bundle` is designed to accept more file classes
  without changing its determinism rules.
- Actually creating a sandbox (driver over `osb`/SDK, volume or push transport,
  Credential Vault bindings) is a later spec. Everything it needs from this one
  is an artifact plus a document.
