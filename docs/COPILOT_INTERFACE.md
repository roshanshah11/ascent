# Copilot Interface (v0.3 Step 9)

The machine contract for AI-native operation. An agent is a *client of the command layer*, exactly like the GUI, the console, and the CLI — it reads document state, proposes command batches, and receives validation plus evidence. It never gets a private mutation path, and nothing it proposes lands without an explicit approval call.

**Constraint (by design):** no network, no LLM runtime inside Ascent. This document specifies the seam; the agent (Claude, Codex, any tool) connects from outside via IPC, the CLI, or a future MCP adapter. Ascent stays offline and deterministic.

## The loop

```
read state ──► propose batch ──► inspect verdict ──► approve ──► apply
   ▲                                                              │
   └────────────── journaled like any other mutation ◄────────────┘
```

1. **Read** — `get_document` returns the full `DocumentState`: vehicle tree, design, studies with results and hashes, undo/redo flags. `session_journal` returns the JSONL session record. Evidence and credibility come from `run_evidence` (scorecard, input hashes, convergence).
2. **Propose** — `propose_commands(lines)` takes a batch of journal-grammar text lines (`docs/JOURNAL_FORMAT.md`). Ascent dry-runs the batch on a clone of the document and returns a `Proposal`. **Nothing mutates.**
3. **Inspect** — the proposal carries per-command errors (with batch indices), the canonical text of what was understood, and — when valid — a `DiffSummary`: what changes, how many parts/studies appear or disappear, and **exactly which studies' stored results would go stale** (`studies_made_stale`). Staleness is computed by input hash, not guessed.
4. **Apply** — `apply_proposal(lines)` re-validates and lands the batch atomically. Any error leaves the document untouched. Applied commands journal normally: they are undoable, replayable, and indistinguishable from GUI edits in the session record.

## Proposal shape

```json
{
  "valid": true,
  "commands": [
    { "index": 0, "text": "set-part-param 3 span_m 0.05", "error": null }
  ],
  "diff": {
    "vehicle_changed": true,
    "design_changed": false,
    "parts_added": 0,
    "parts_removed": 0,
    "studies_added": 0,
    "studies_removed": 0,
    "studies_made_stale": [1]
  }
}
```

An invalid batch sets `valid: false`, omits `diff`, and reports **every** command's error in one round-trip — each command is checked against the state the previous successful commands produced, so an agent fixes the whole batch at once instead of discovering errors one by one.

## Rules for agent clients

- **Batches are the unit of intent.** One proposal should be one coherent change ("widen the fins and re-select the motor"), not a grab-bag. The human approving reads the diff summary.
- **Watch `studies_made_stale`.** Proposing a geometry edit that stales a validated study is legal, but the agent should surface that to the approver — that list is the physics-in-the-loop warning.
- **Never bypass.** `RestorePart`/`RestoreStudy` exist for undo fidelity; agents send the ordinary forms and let the dispatcher allocate ids.
- **Verify after apply.** Re-read state and compare against the proposal's diff. Hashes make drift detectable; use them.
- **Headless agents** use the same grammar through `ascent-cli` (`replay`, `run-study`) — output is hash-stamped JSON, comparable byte-for-byte with GUI results.

## Entry points

| Surface | Call | Mutates? |
|---|---|---|
| IPC | `get_document`, `session_journal`, `run_evidence` | no |
| IPC | `propose_commands(lines)` | no |
| IPC | `apply_proposal(lines)` | yes — atomic, journaled |
| IPC | `console_exec(line)` | yes — single command, journaled |
| CLI | `ascent-cli replay`, `ascent-cli run-study` | no (headless artifacts) |

Implementation: `crates/ascent-app/src/propose.rs` (tested), grammar in `crates/ascent-app/src/command.rs`.
