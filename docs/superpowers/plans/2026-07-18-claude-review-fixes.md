# Claude Review Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Correct the v0.4 frontend, command catalogue, MCP server, and workspace-discipline defects recorded in `docs/CLAUDE_BUILD_REVIEW.md`.

**Architecture:** Keep rendering and tool clients as views over backend-owned state. Lazy-load the R3F island, expose the Rust command catalogue read-only, replace hand-written MCP framing with official RMCP stdio transport, and keep crate metadata explicit.

**Tech Stack:** React 19, TypeScript, Vite, Tauri v2, Rust 2021/MSRV 1.97, RMCP 2.2, Tokio stdio, Vitest, Cargo tests.

## Global Constraints

- Preserve deterministic simulation and the byte-identical journal.
- Every mutation continues through `Document::dispatch` or the existing atomic proposal seam.
- The render layer dispatches zero commands.
- No runtime network transport or client is created.
- No component framework and no changes to external journal command text.
- Preserve all pre-existing uncommitted work, especially simulator, structural, STL, and eval changes.

---

### Task 1: Lazy and visually finish the R3F workbench

**Files:**
- Modify: `app/src/App.tsx`
- Modify: `app/src/App.test.tsx`
- Modify: `app/src/styles/theme.css`
- Test: `app/src/App.test.tsx`

**Interfaces:**
- Consumes: `ViewportR3F` default export and existing `Vehicle` / `VehicleMarkers` props.
- Produces: a separate Vite viewport chunk and `.design-workspace`, `.viewport-panel`, `.viewport-r3f`, `.viewport-loading` layout contracts.

- [ ] **Step 1: Write failing frontend tests**

Add tests that select 3D and assert a scoped `Loading 3D viewport…` fallback, plus a source/build assertion that the viewport module is not statically imported by the initial app module.

- [ ] **Step 2: Verify the tests fail**

Run: `rtk npm test -- App.test.tsx`
Expected: FAIL because `ViewportR3F` is statically imported and no lazy fallback exists.

- [ ] **Step 3: Implement the lazy boundary and named layout**

Use `lazy(() => import("./components/ViewportR3F"))` and `Suspense fallback={<div className="viewport-loading">Loading 3D viewport…</div>}`. Replace the Design workspace's outer inline flex container with `<section className="design-workspace">` and `<div className="viewport-panel">`.

- [ ] **Step 4: Complete shared visual rules**

Add explicit responsive viewport sizing (`min-height: clamp(360px, 58vh, 760px)`), grid columns, panel surfaces, button/input states, `:focus-visible`, disabled states, and a narrow-width media query. Keep palette-specific rules intact.

- [ ] **Step 5: Verify focused behavior and bundle split**

Run: `rtk npm test -- App.test.tsx && rtk npm run build`
Expected: tests PASS; build emits an initial app chunk plus a separate `ViewportR3F` chunk and no initial 1.18 MB monolith.

---

### Task 2: Make command discovery Rust-owned

**Files:**
- Modify: `crates/ascent-app/src/command.rs`
- Modify: `crates/ascent-app/src/lib.rs`
- Modify: `app/src/ipc.ts`
- Modify: `app/src/App.tsx`
- Modify: `app/src/components/CommandPalette.tsx`
- Modify: `app/src/components/CommandPalette.test.tsx`
- Delete: `app/src/core/grammarCommands.ts`
- Test: `crates/ascent-app/src/command.rs`

**Interfaces:**
- Produces: `pub struct CommandGrammarEntry { pub verb: &'static str, pub usage: &'static str, pub description: &'static str }` and `pub fn command_grammar() -> &'static [CommandGrammarEntry]`.
- Produces: Tauri `command_catalogue() -> Vec<CommandGrammarEntry>` and TypeScript `fetchCommandCatalogue(): Promise<GrammarCommand[]>`.

- [ ] **Step 1: Write failing Rust catalogue tests**

Assert unique verbs, the exact 12 current parser verbs, and `parse_text` coverage for one valid representative line per catalogue entry. Add a console/journal test that dispatches `select-motor B6`, finds the canonical entry, replays it, and compares canonical bytes.

- [ ] **Step 2: Verify Rust tests fail**

Run: `rtk cargo test -p ascent-app command::tests::command_grammar`
Expected: FAIL because the catalogue API does not exist.

- [ ] **Step 3: Implement and expose the catalogue**

Define the serializable entry and single static catalogue beside `Command::parse_text`. Register the read-only Tauri command and frontend IPC function. Preserve all existing parser text exactly.

- [ ] **Step 4: Write failing palette injection tests**

Update `CommandPalette.test.tsx` to provide a `grammarCommands` prop and assert supplied verbs render. Add an App test for a catalogue-fetch rejection that leaves named actions usable and shows a visible error.

- [ ] **Step 5: Remove the TypeScript mirror**

Fetch catalogue during App initialization, inject it into the palette, and delete `app/src/core/grammarCommands.ts`. Do not add a fallback static list.

- [ ] **Step 6: Verify both sides**

Run: `rtk cargo test -p ascent-app command && rtk npm test -- CommandPalette.test.tsx App.test.tsx`
Expected: all focused tests PASS.

---

### Task 3: Migrate ascent-mcp to official RMCP stdio

**Files:**
- Modify: `crates/ascent-mcp/Cargo.toml`
- Replace: `crates/ascent-mcp/src/lib.rs`
- Modify: `crates/ascent-mcp/src/main.rs`
- Replace: `crates/ascent-mcp/tests/roundtrip.rs`
- Preserve/extend: `crates/ascent-mcp/tests/evals.rs`

**Interfaces:**
- Consumes: `apply_batch`, `propose_batch`, `run_study_now`, `evidence_for`, `Document`, and `StudyId` from `ascent-app`.
- Produces: `AscentMcp` implementing RMCP `ServerHandler`, with the same five public tool names and stdio-only binary startup.

- [ ] **Step 1: Resolve and record RMCP 2.2 APIs**

Use official docs for `#[tool]`, `#[tool_router]`, `ServerHandler`, `ServiceExt::serve`, and `transport::io::stdio`. Enable only `server`, `macros`, and `transport-io` features plus the required Tokio runtime features.

- [ ] **Step 2: Write failing SDK-level tests**

Create tests that initialize an RMCP client/server pair, list the exact five tools, reject `study_id = 4294967296`, and propose/apply a batch that reports `studies_made_stale`.

- [ ] **Step 3: Verify tests fail against the hand-written server**

Run: `rtk cargo test -p ascent-mcp`
Expected: FAIL because `AscentMcp: ServerHandler` and RMCP transport setup do not exist.

- [ ] **Step 4: Implement the five SDK tools**

Use typed `Json<T>` / `Parameters<T>` schemas. Keep document state behind a server-owned lock, parse command lines through `Command::parse_text`, and return SDK-standard tool errors. Convert study IDs with `u32::try_from`.

- [ ] **Step 5: Implement truthful study execution**

First verify whether RMCP 2.2 exposes the 2026-07-28 Tasks extension. If it does, connect `run_study` to a non-blocking worker with queued/running/completed/failed/cancelled states. If it does not, advertise no Tasks capability and keep `run_study` synchronous; remove every `tasks/*` façade and task claim.

- [ ] **Step 6: Use stdio only and prove it**

Start the binary with `stdio()` and `serve`. Add a source/dependency assertion that no HTTP transport feature, listener, or client is enabled.

- [ ] **Step 7: Verify MCP behavior**

Run: `rtk cargo test -p ascent-mcp`
Expected: all SDK, eval, overflow, initialization, and stdio tests PASS.

---

### Task 4: Complete workspace metadata and crate contracts

**Files:**
- Modify: `crates/ascent-aero/Cargo.toml`
- Modify: `crates/ascent-app/Cargo.toml`
- Modify: `crates/ascent-review/Cargo.toml`
- Modify: each workspace `src/lib.rs` lacking crate-level documentation

**Interfaces:**
- Produces: `license.workspace = true` for every crate and concise `//!` public-surface contracts.

- [ ] **Step 1: Write a failing metadata assertion**

Add an `xtask` check or focused test that parses `cargo metadata --no-deps` and fails if any workspace package has no license.

- [ ] **Step 2: Verify it fails**

Run: `rtk cargo metadata --no-deps --format-version 1`
Expected inspection: `ascent-aero`, `ascent-app`, and `ascent-review` report `license: null` before the fix.

- [ ] **Step 3: Add inheritance and crate docs**

Add `license.workspace = true` to the three manifests. Add `//!` headers describing intended consumers, supported exports, and private implementation modules without widening exports.

- [ ] **Step 4: Verify metadata and docs**

Run: `rtk cargo metadata --no-deps --format-version 1 && rtk cargo doc --workspace --no-deps`
Expected: every workspace package reports MIT and docs build without warnings.

---

### Task 5: Integration, visual QA, and final review

**Files:**
- Modify only files required by findings from the final review.

**Interfaces:**
- Consumes all prior task outputs; produces a verified branch-ready diff.

- [ ] **Step 1: Run the complete gate**

Run: `rtk cargo xtask test`
Expected: all Rust, TypeScript, and Vitest suites PASS.

- [ ] **Step 2: Verify production performance**

Run: `rtk npm run build`
Expected: exit 0; viewport dependencies are isolated from the initial app chunk.

- [ ] **Step 3: Inspect the UI visually**

Run the Tauri/Vite surface and inspect desktop plus narrow-width Design views, 2D/3D transition, lazy fallback, keyboard focus, disabled states, and CP/CG labels.

- [ ] **Step 4: Audit scope and formatting**

Run: `rtk proxy git diff --check` and compare status against the pre-task dirty-file inventory.
Expected: no whitespace errors and no simulator/STL/structural/eval work overwritten.

- [ ] **Step 5: Complete whole-branch code review**

Review against `docs/superpowers/specs/2026-07-18-claude-review-fixes-design.md`; fix all Critical/Important findings and rerun covering tests.
