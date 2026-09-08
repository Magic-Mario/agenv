# Dependency metadata + flat `[skills]` serialization — Design

## Data model (non-breaking)

Manifest `agenv.toml` — before:

```toml
name    = "payments-backend"
harness = ["claude-code"]

[skills.rust-reviewer]
source = "https://github.com/acme/skills"
ref    = "v1.2.0"
```

After:

```toml
name    = "payments-backend"
harness = ["claude-code"]

[skills]
"rust-reviewer" = { source = "https://github.com/acme/skills", ref = "v1.2.0", description = "Reviews Rust", license = "MIT", version = "1.2.0", homepage = "https://example.com" }
"grill-with-docs" = { source = "https://github.com/mattpocock/skills", ref = "main", path = "skills/engineering/grill-with-docs" }
```

The `skills` map key stays the skill name; only the TOML layout changes from
per-skill sub-tables to inline tables. The Rust model is unchanged except for
the added metadata fields:

```rust
pub struct Manifest {
    pub name: String,
    pub harness: Vec<String>,
    #[serde(default)]
    pub skills: BTreeMap<String, SkillSpec>,  // unchanged
}

pub struct SkillSpec {
    pub source: String,
    pub r#ref: String,
    pub path: Option<String>,
    pub description: Option<String>,  // new
    pub license: Option<String>,      // new
    pub version: Option<String>,      // new
    pub homepage: Option<String>,     // new
}
```

Because `toml::from_str` treats `"name" = { … }` and `[skills.name]` as the same
table, existing manifests keep loading with no migration. The first save after
this change rewrites them flat.

## Serialization

`Manifest::save` switches from `toml::to_string_pretty` (which emits nested
`[skills.<name>]` tables) to `toml_edit`, building one inline table per skill:

```rust
let mut doc = toml_edit::DocumentMut::new();
doc["name"] = toml_edit::value(self.name.clone());
// harness: array of strings
let mut skills = toml_edit::Table::new();
for (name, spec) in &self.skills {
    let mut t = toml_edit::InlineTable::new();
    t.insert("source", spec.source.clone().into());
    t.insert("ref", spec.r#ref.clone().into());
    // path + metadata only when set
    skills.insert(name, toml_edit::Item::Value(toml_edit::Value::InlineTable(t)));
}
doc["skills"] = toml_edit::Item::Table(skills);
```

- An empty `skills` map emits no `[skills]` section.
- `toml_edit` is already a transitive dependency of `toml` (via its default
  `parse`/`display` features), so adding it as a direct dependency costs no
  extra compile time.
- Deserialization stays on `toml::from_str`; no new parsing code.

## Validation

`validate_skill(&name, &spec)` keeps its signature (name stays a map key) and
gains metadata checks alongside the existing source/ref/path rules:

- `is_semver(s)` — split core on `-`/`+`, require three numeric dot components.
- `is_https_url(s)` — `s.starts_with("https://")`.
- non-empty `description` / `license` when present.

## `add` flags

`add` gains `--description`, `--license`, `--version`, `--homepage`, threaded
through `commands::add::run` into the `SkillSpec`. Upsert stays
`manifest.skills.insert(name, spec)`.

## Surfacing (cross-spec, deferred)

`status`/`diff` render `description` on the skill's human row and the full
metadata set in JSON `Finding`. This lands after the `status` spec's `Finding`
model; not part of this spec's first pass.

## Files touched

| File | Change |
|---|---|
| `Cargo.toml` | add `toml_edit = "0.22"` |
| `src/manifest.rs` | `SkillSpec` + 4 optional metadata fields (+ `Default`); `save` via `toml_edit` inline tables; `is_semver`/`is_https_url`; wire into `validate_skill`; update unit tests |
| `src/commands/add.rs` | thread 4 metadata flags into the `SkillSpec` |
| `src/main.rs` | `Add` subcommand gains `--description --license --version --homepage` |
| `src/commands/sync.rs` | `SkillSpec { ..Default::default() }` for the 3 construction sites |
| `tests/e2e.rs` | `[skills.stray]` assertion → flat `[skills]` + `"stray"`; add a metadata round-trip assertion |
| `README.md` | flat `[skills]` format + metadata fields |
| `specs/schema-validation` (`agenv.schema.json`) | flat `[skills]` inline-table schema (cross-spec, R9) |

## Cross-spec dependencies

This spec owns the manifest file layout and the `SkillSpec` metadata fields.
`status`/`diff`/`doctor`/`schema-validation` read them but are unaffected by the
serialization change itself; they land after, against the final shape
(`SkillSpec` with optional metadata).
