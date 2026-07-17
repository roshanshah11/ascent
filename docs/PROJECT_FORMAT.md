# The `.ascent` Project Format

*v0.2 Step 1 (plan: `docs/plan/2026-07-17-v02-professionalization.md#step-1`). Implementation: `crates/ascent-app/src/project.rs`.*

## Design goals

The `.ascent` file is the anti-`.ork`: **plain TOML, human-readable, git-diffable**. OpenRocket's zipped-XML format is the documented pain that pushes teams into MATLAB bridges and un-mergeable binary blobs in their repos. One field edit in an `.ascent` file is one changed line in `git diff`— enforced by test (`one_field_edit_is_a_one_line_diff`).

## Shape (schema_version 1)

```toml
schema_version = 1
name = "regression bird"

[[designs]]
name = "Estes Alpha III"
dry_mass_g = 34.0
diameter_mm = 25.0
cd = 0.6
motor_designation = "C6"
rail_length_m = 0.9

[designs.chute]
enabled = true
diameter_cm = 30.0
cd = 0.75

[[runs]]
# full RunRecord: design snapshot + SimSummary (incl. input_hash) + events + samples
```

- `designs` — every rocket in the project (multi-design workspace lands with the UI work; the model supports it now).
- `runs` — complete `RunRecord`s. Each embeds the design *as it was at run time* plus the SHA-256 `input_hash`, so a loaded project can prove whether a stored result is stale for the current design without re-simulating.

## Versioning policy

- `schema_version` bumps **only on breaking changes**.
- **Additive changes do not bump**: readers tolerate unknown fields (tested: `unknown_fields_are_tolerated`). A v0.2 build opens a file written by a v0.3 build that only *added* fields.
- A file with `schema_version` greater than the reader's supported version is rejected with a message naming both versions and telling the user to update (tested: `newer_schema_version_is_rejected_with_clear_message`).
- Roundtrip is byte-identical (`to_toml → from_toml → to_toml`), so save-without-edit never dirties a git working tree.

## IPC

- `save_project(project) -> String` — serialize; the frontend owns the file dialog / disk write (Tauri fs scope).
- `load_project(contents) -> Project` — parse + validate.

Autosave/crash recovery is Step 8 and layers on top of this module without format changes.
