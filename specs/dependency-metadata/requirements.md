# Dependency metadata + flat `[skills]` serialization — Requirements

## Overview
Two changes to `agenv.toml`:

1. **Flat serialization** — the `skills` map is emitted as a single `[skills]`
   table whose values are inline tables (`"name" = { … }`), replacing the old
   per-skill sub-tables (`[skills.<name>]`). The Rust model stays a name-keyed
   map; only the file layout changes.
2. **Dependency metadata** — each skill entry gains optional
   `description`/`license`/`version`/`homepage` fields.

This is **not** a breaking model change: both the old nested layout and the new
flat layout deserialize into the same `BTreeMap<String, SkillSpec>`, so existing
manifests keep loading.

## Goals
- A flat, readable manifest: one `[skills]` table, every skill an inline entry.
- Name uniqueness stays enforced by the map (no explicit duplicate check).
- Optional, structured metadata per skill, set from `add`, validated, and
  preserved through every command.

## Non-goals
- Re-keying the manifest to `[[skills]]` (an ordered array) — rejected in favor
  of keeping the name-keyed map and only flattening the serialization.
- Reading metadata out of `SKILL.md` frontmatter (deferred — would add a YAML
  parser).
- A registry / package index; enforcing license/version policy beyond shape.

## Requirements

### Manifest serialization (flat `[skills]`)
- **R1** — `agenv.toml` SHALL serialize `skills` as a single `[skills]` table
  whose values are inline tables, e.g.
  ```toml
  [skills]
  "my-skill" = { source = "https://…", ref = "main", path = "…" }
  ```
  No `[skills.<name>]` sub-tables are emitted.
- **R2** — The Rust model SHALL stay `BTreeMap<String, SkillSpec>`; `name`
  remains the map key. Old nested `[skills.<name>]` manifests SHALL still load
  (same TOML semantics) and, on next save, be rewritten flat.
- **R3** — An empty `skills` SHALL be omitted from the file (no empty
  `[skills]` table); loading a manifest with no `skills` yields an empty map.
- **R4** — Serialization SHALL be produced by `Manifest::save` via `toml_edit`
  (inline tables), while deserialization keeps `toml::from_str`.

### Metadata
- **R5** — `SkillSpec` SHALL gain optional fields `description`, `license`,
  `version`, `homepage`, each `Option<String>`, `#[serde(default,
  skip_serializing_if = "Option::is_none")]` — omitted when unset, round-trip
  when set.
- **R6** — Validation: `version` when present SHALL match a loose semver shape
  (`<major>.<minor>.<patch>` with optional pre/build); `homepage` when present
  SHALL start with `https://`; `description` and `license` when present SHALL be
  non-empty. Enforced by `validate_skill`.
- **R7** — `add` SHALL accept `--description`, `--license`, `--version`,
  `--homepage` and write them into the entry; `add` SHALL upsert by name over
  the map (unchanged).
- **R8** — Metadata SHALL be preserved by install/sync/update (manifest intent;
  lock entries stay commit+checksum and are unchanged).
- **R9** — `agenv.schema.json` (owned by the `schema-validation` spec) SHALL
  describe the flat `[skills]` inline-table format plus the metadata fields.

## Acceptance criteria
- R1: `agenv init` then adding two skills yields a `[skills]` table with two
  inline entries; no `[skills.<name>]` header appears in the file.
- R2: a fixture in the old nested `[skills.foo]` format loads and, after any
  command that saves, is rewritten flat.
- R3: `agenv init` writes no `[skills]` section at all.
- R4: round-trip unit test (save → load) passes with inline output.
- R5: round-trip unit test: set all four metadata fields, save, load, equal.
- R6: `version = "not-a-version"` rejected; `homepage = "ftp://x"` rejected;
  empty `description` rejected.
- R7: `add --description "…" --version "1.0.0"` writes them in the inline entry.
- R8: `install`/`sync`/`update` leave metadata intact in `agenv.toml`.

## Implementation order (cross-spec)

The serialization change is self-contained (only `Manifest::save` plus the
`toml_edit` dependency). Metadata builds on top:

1. Flat serialization (T1) — the only structural change.
2. Metadata fields + validation + `add` flags (T2–T4).
3. Cross-spec surfacing (`status`/`diff`) and schema (`schema-validation`) land
   after, reading the metadata fields.

`status`, `diff`, `doctor`, and `schema-validation` read the post-change model
(`SkillSpec` with optional metadata); none of them are affected by the flat
serialization itself.
