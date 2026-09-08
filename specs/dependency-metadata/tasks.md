# Dependency metadata + flat `[skills]` serialization — Tasks

## Phase 1 — Flat serialization

- [x] **T1** — `Cargo.toml`: add `toml_edit = "0.22"`. `src/manifest.rs`: rewrite
  `Manifest::save` to emit `[skills]` with inline tables via `toml_edit`
  (empty skills → no section); keep `Manifest::load` on `toml::from_str`.
  Update the `round_trip` unit test to assert the inline format (R1, R3, R4).
  - depends on: none
  - result: PASS — `Cargo.toml` has `toml_edit = "0.22"`; `save` builds an `InlineTable` per skill under `[skills]` and skips the section when empty; `load` uses `toml::from_str`. `round_trip`/`empty_skills_omits_section` tests assert inline format + omitted empty section. All pass.

## Phase 2 — Metadata

- [x] **T2** — `src/manifest.rs`: add `description`/`license`/`version`/
  `homepage` to `SkillSpec` (Option<String>, serde default + skip); derive
  `Default`; serialize them inline in `save`. Round-trip unit test (R5).
  - depends on: T1
  - result: PASS — `SkillSpec` has the four `Option<String>` fields with `#[serde(default, skip_serializing_if = "Option::is_none")]`, derives `Default`, and `save` inserts each inline. `round_trip` test populates all four fields and round-trips.
- [x] **T3** — `src/manifest.rs`: `is_semver` + `is_https_url`; wire into
  `validate_skill` (non-empty description/license, semver version, https
  homepage). Unit tests (R6).
  - depends on: T2
  - result: PASS — `is_semver`/`is_https_url` exist and run via `validate_metadata`, called from `validate_skill` before the local-source early return (so local sources are validated too). Tests `metadata_validation` + `local_source_metadata_is_validated` cover it.
- [x] **T4** — `src/commands/add.rs` + `src/main.rs`: `--description --license
  --version --homepage` flags threaded into the `SkillSpec` (R7).
  - depends on: T2
  - result: PASS — `main.rs` declares the four `#[arg(long)]` flags on `Add` and passes them via `commands::add::Meta`; `add.rs` copies them into `SkillSpec`. Verified by `add_metadata_round_trips` e2e.
- [x] **T5** — `src/commands/sync.rs`: `SkillSpec { ..Default::default() }` at
  the three construction sites. Confirm install/sync/update preserve metadata
  (R8).
  - depends on: T2
  - result: PASS — `sync.rs` uses `..Default::default()` at the 3 `adopt` sites (lock/local/vendor). `install.rs`/`update.rs` only read `manifest.skills` and never rebuild specs, so metadata survives; `sync` only inserts new skills. Confirmed by `add_metadata_round_trips` asserting homepage still present after install.

## Phase 3 — Tests & docs

- [x] **T6** — `tests/e2e.rs`: replace the `[skills.stray]` assertion with the
  flat `[skills]` + `"stray"` inline form; add an e2e assertion that a metadata
  `add` round-trips through install (R1, R7, R8).
  - depends on: T4
  - result: PASS — `sync_adopts_harness_skill_vendors` now asserts `[skills]` + `stray = {` (no `[skills.stray]`), and `add_metadata_round_trips` exercises the metadata flags and asserts description/version/homepage survive `add`+`install`. Both pass.
- [x] **T7** — `README.md`: document the flat `[skills]` format and metadata
  fields.
  - depends on: T1
  - result: PASS — README documents the flat `[skills]` table with inline `"name" = { source, ref, path? }` + optional `description`/`license`/`version`/`homepage` (Files section), the `--description/--license/--version/--homepage` flags, and the metadata validation rules (Behavior notes).

## Cross-spec notes
- `agenv.schema.json` (R9) is owned by `specs/schema-validation`; it must
  describe the flat `[skills]` inline-table format and is a follow-up there.
- Metadata surfacing in `status`/`diff` is a follow-up in the `status` spec.
