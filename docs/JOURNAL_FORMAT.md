# Journal Format (v0.3 Step 2)

The journal is the session's memory and the product's automation contract: every mutation of user-owned state is a named `Command` dispatched through the one dispatcher in `crates/ascent-app/src/document.rs`. The GUI, the scripting console, the headless CLI, and the copilot seam are all clients of this stream — there is no second mutation path.

## File shape

JSONL. Line 1 is the header; every following line is one operation, in dispatch order.

```
{"journal_version":1}
{"op":"dispatch","command":{"cmd":"set_sim_param","param":"cd","value":0.7}}
{"op":"dispatch","command":{"cmd":"add_part","parent":2,"kind":{"type":"fin_set","count":4,"root_chord_m":0.04,"tip_chord_m":0.02,"span_m":0.03,"sweep_m":0.0,"thickness_mm":2.0,"mass_g":5.0}}}
{"op":"undo"}
{"op":"redo"}
```

- **Undo and redo are journaled** — a record that skipped them could not reproduce the final state.
- **Replay** (`Document::replay`) starts from the default document and re-dispatches every line. Replay must reproduce the live document **byte-identically** (`canonical_bytes()` equality) — the determinism discipline extends to the command layer.
- Errors carry 1-based line numbers. An unknown `journal_version` fails immediately.

## Command grammar (`cmd` tag)

| Command | Fields | Notes |
|---|---|---|
| `add_part` | `parent` (PartId or null), `kind` (tagged PartKind per `VEHICLE_TREE.md`) | Id allocated at apply time from a monotonic counter; counter state is part of the document, so replay allocates identically. |
| `remove_part` | `id` | Inverse restores the exact part at its exact index. |
| `restore_part` | `parent`, `index`, `part` | Concrete re-insertion; appears in journals only via replay of undo/redo internals — normal clients never send it. |
| `set_part_param` | `id`, `param`, `value` | `param` is the serde field name. `as_built_mass_g` is also accepted at the part wrapper (a non-negative number or `null` to restore the design mass); every other parameter patches `PartKind`. Typo'd params and wrong types are rejected atomically. `type` is immutable. |
| `set_sim_param` | `param`, `value` | Flat design fields; dotted `chute.*` paths for the chute. Dual-deploy adds `chute.main_deploy_altitude_m`, `chute.drogue_diameter_cm`, `chute.drogue_cd` — each optional (absent = single-deploy) and settable from unset; pass `null` to clear one back to single-deploy. `motor_designation` is refused — use `select_motor`. |
| `select_motor` | `designation` | |
| `set_design` | `design` | Coarse replacement: reset and crash-recovery restore. |
| `batch` | `commands` | One validated approval unit. Nested batches are rejected; one undo restores the entire batch. |
| `set_telemetry` | `bundles` | Replaces immutable raw and normalized telemetry evidence inline. |
| `set_alignment` | `alignment` or null | Selects an explicit clock relationship without rewriting source timestamps. |
| `set_reconciliation` | `reconciliation` or null | Lands phase-aware residual evidence and its selected alignment. |

Dispatch records may carry a `metadata` object with `author`, `source`, `intent`, `affected_requirements`, and `evidence_hashes`. All fields are serde-defaulted, so journals written before v0.6 remain valid. Campaign cinema exposes this metadata but never appends to or mutates the source journal.

## Semantics

- **Atomic**: a command either fully applies (and the tree still passes `validate()`) or the document is untouched and an error string returns.
- **Invertible**: `apply` returns a concrete forward form plus its inverse. `add_part`'s concrete forward is a `restore_part`, so redo reproduces the identical id instead of allocating a fresh one.
- **Monotonic ids**: `PartId`s are never reused within a document, so parts referenced from undo/redo stacks can never collide.
- **Versioned**: bump `journal_version` on any breaking grammar change; readers refuse unknown versions with a clear message.

## Text form (Step 8)

The scripting console and the headless CLI share one canonical text form per command, implemented in `crates/ascent-app/src/command.rs` (`Command::to_text` / `Command::parse_text`). Both parse to the same `Command` enum and go through the same dispatcher — there is no second mutation path. `parse_text(to_text(cmd)) == cmd` for every variant (tested).

Shape: a kebab-case verb, simple positional tokens, JSON for structured payloads. `-` stands for a null parent.

| Text form | Command |
|---|---|
| `add-part <parent\|-> <kind-json>` | `add_part` |
| `remove-part <id>` | `remove_part` |
| `restore-part <parent\|-> <index> <part-json>` | `restore_part` |
| `set-part-param <id> <param> <value-json>` | `set_part_param` |
| `set-sim-param <param> <value-json>` | `set_sim_param` |
| `select-motor <designation>` | `select_motor` |
| `set-design <design-json>` | `set_design` |
| `create-study <"name"> <engine> <seed> <kind-json>` | `create_study` |
| `delete-study <id>` | `delete_study` |
| `restore-study <index> <study-json>` | `restore_study` |
| `set-study-param <id> <param> <value-json>` | `set_study_param` |
| `set-study-results <id> <results-json\|null>` | `set_study_results` |
| `set-atmosphere <profile-json\|null>` | `set_atmosphere` |
| `set-telemetry <bundles-json>` | `set_telemetry` |
| `set-alignment <artifact-json\|null>` | `set_alignment` |
| `set-reconciliation <result-json\|null>` | `set_reconciliation` |
| `batch <commands-json>` | `batch` |

Example: `set-part-param 3 root_chord_m 0.05` · `create-study "landing spread" native 42 {"kind":"dispersion","flights":1000}`

## Headless CLI (Step 8)

`ascent-cli replay <session.jsonl>` rebuilds a document from a journal and prints its canonical JSON — byte-equality with the live session's `canonical_bytes()` is the determinism proof, and the CI story. `ascent-cli run-study <project.ascent> <study-id-or-name>` runs a study headless and prints hash-stamped results JSON (the same `input_hash` the GUI's job runner stamps).
