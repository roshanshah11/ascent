# Claude build review: v0.4 workbench

*Live review begun 2026-07-18. This is a running correction log, not a request to discard the work. Findings are based on the current worktree, `npm run build`, `npm run test`, `cargo test -p ascent-app`, and current Context7 documentation for React Three Fiber, drei, and Three.js.*

## Highest-priority corrections

### 1. Keep the 3D stack off the default startup path

**Evidence.** `npm run build` currently emits a single JavaScript asset of **1,182.25 kB** (327.32 kB gzip) and Vite emits its `>500 kB` chunk warning. `App.tsx` statically imports `ViewportR3F`, even though the app opens in the 2D Design view (`view3d === false`). That makes every default startup download, parse, and compile Three, R3F, and drei.

**Correction.** Load `ViewportR3F` with `React.lazy` and render it inside a narrow `Suspense` boundary only when the user chooses 3D. Keep the 2D shell, inspector, and palette in the initial chunk. Rebuild and record the initial-chunk size plus the separate viewport chunk size. This preserves the richer viewport without making the basic workbench slower.

**Why it matters.** R3F's `frameloop="demand"` controls render work *after* the bundle is loaded. It does not address the measured startup payload.

### 2. Give the R3F viewport an explicit, responsive size

**Evidence.** `ViewportR3F.tsx` renders `<div className="viewport-r3f">`, but `app/src/styles/theme.css` contains no `viewport-r3f` rule. The Canvas is configured as 100%-sized inside an auto-height parent, leaving its rendered height dependent on browser defaults rather than the workbench layout.

**Correction.** Define the viewport as a real panel, e.g. `min-height: clamp(360px, 58vh, 760px); width: 100%;`, with overflow and a border/background that belongs to the visual system. Make the Design workspace grid allocate that panel deliberately, then check the view at desktop and narrow widths.

**Why it matters.** A 3D centerpiece with an accidental or tiny canvas will fail the stated visual bar regardless of its mesh quality.

### 3. Make the palette's command catalogue backend-owned

**Evidence.** Rust owns `Command::to_text` / `Command::parse_text` in `crates/ascent-app/src/command.rs`, but the palette uses a hand-maintained `app/src/core/grammarCommands.ts` mirror. The TypeScript comment explicitly accepts that a stale entry may fail to suggest a valid command. This conflicts with the requirement to fuzzy-search the canonical command grammar and eventually creates a silent feature gap whenever a command is added.

**Correction.** Expose a read-only `command_grammar` Tauri command generated from a single Rust `Command::grammar()` definition (verb, usage, description), or generate a checked-in frontend manifest from that definition. Have the palette consume it, and add a Rust test that every parser verb appears exactly once in the manifest.

**Why it matters.** The palette must be a view of the grammar, not a second grammar implementation. Dispatching through `console_exec` is correct, but it only protects execution after a command has been discovered.

## Important quality corrections

### 4. Finish the mission-control visual system instead of styling only the new chrome

**Evidence.** `theme.css` defines tokens plus workbench/palette styles, but it has no shared `button`, focus-visible, panel, viewport, or responsive workspace rules. Most existing controls still render from JSX with browser-default buttons and inline layout styles. The result will read as a dark shell around an older prototype rather than one coherent workbench.

**Correction.** Add a small global baseline for buttons, inputs, focus rings, scrollbar/panel surfaces, and disabled states; move the Design workspace into named grid/panel classes; then remove only the inline layout rules touched by the shell. Keep semantic HTML and avoid a component framework.

**Why it matters.** This is the highest-leverage aesthetic pass: consistent hierarchy, contrast, spacing, and keyboard focus are more convincing than adding decoration.

### 5. Test the stated acceptance paths, not only helpers and mocked callbacks

**Evidence.** The current frontend suite passes (**93 tests**), and the complete `cargo xtask test` gate passes. The new palette component test proves its mocked `onExec` callback is called for one line, but it does not prove an actual palette command reaches `console_exec` and becomes a journal entry. `ViewportR3F` has no direct rendered/component test; the new mesh-group tests cover only pure grouping math.

**Correction.** Add a focused Rust/Tauri integration test that calls `console_exec`, then inspects `session_journal` for the canonical line and verifies replay. For the viewport, add a lightweight Canvas-mocked component test covering CP/CG marker data and the 2D/3D toggle, while retaining pure mesh tests. Keep the full suite and production build as gates.

**Why it matters.** The acceptance criteria are command journaling, read-only rendering, and a live tree-derived viewport. Unit tests alone are not evidence for those boundaries.

### 6. Do not claim an asynchronous MCP Tasks implementation when each task has already run synchronously

**Evidence.** `ascent-mcp` executes `run_study_now` inside `tools/call`, then, only when a `task` parameter is present, stores the *already-completed* result and returns `status: "completed"`. The same process cannot receive a cancel request or return progress until the simulation is over. `tasks/cancel` always returns `task already completed`, yet the tool description says task-augmented calls support long dispersions.

**Correction.** Either keep `tasks` out of the advertised capability catalog until there is a real background job/state machine with queued/running/completed/failed/cancelled states, or implement it against the existing job-runner seam. The task path must return immediately, progress must be observable, cancellation must affect queued/running work, and `tasks/result` must only become available at completion. Add a round-trip test that demonstrates those transitions.

**Why it matters.** A synchronous call wrapped in a completed task changes protocol vocabulary, not latency or operability. It is especially misleading for the exact long-dispersion case Tasks was introduced to solve.

### 7. Re-evaluate the SDK decision with the actual stdio surface, not an assumed networking cost

**Evidence.** `crates/ascent-mcp/Cargo.toml` rejects `rmcp` because Tokio is described as an “async-network runtime” that conflicts with the no-network invariant. Current official RMCP documentation exposes a server-capable `transport::io::stdio()` transport and describes stdio as a built-in server transport. An async runtime is not a network connection.

**Correction.** Prototype the five tools with RMCP's stdio-only server features and compare the resulting dependency/build cost against the hand-written protocol. If the dependency cost still loses, retain the hand-roll only after adding protocol-conformance coverage: initialize gating, initialize negotiation, tool schemas, task lifecycle, cancellation, and malformed-message handling. Do not let “no runtime network” be used as a reason to reject a local stdio SDK without evidence.

**Why it matters.** MCP protocol churn is precisely where an official SDK buys correctness. A small local server is a reasonable exception, but the exception needs a technically true justification.

### 8. Enforce MCP initialization and validate numeric narrowing at the protocol boundary

**Evidence.** `McpServer` tracks `initialized` but never reads it, so `tools/list` and `tools/call` succeed before `initialize`. `run_study` accepts a JSON `u64` and casts it directly to `u32`, silently wrapping values above `u32::MAX` into a different study id.

**Correction.** Reject all non-initialize requests before a successful initialization (aside from any specification-required probes), and use `u32::try_from(sid)` with a clear invalid-parameter error. Add exact tests for both conditions.

**Why it matters.** These are cheap boundary checks that prevent confusing client failures and make the MCP seam safe to automate.

### 9. Complete the workspace metadata and public-surface documentation pass

**Evidence.** The workspace declares `license = "MIT"`, but `ascent-aero`, `ascent-app`, and `ascent-review` do not inherit it with `license.workspace = true`; Cargo metadata reports their license as `null`. Their crate roots also begin directly with module/export declarations rather than a crate-level description of the intended public surface, despite this being an explicit workspace-pass acceptance item.

**Correction.** Add `license.workspace = true` consistently to every private crate manifest, and start each `src/lib.rs` with a concise `//!` contract describing consumers, stable exports, and intentionally internal modules. Keep the export lists curated as they are.

**Why it matters.** A workspace pass should make the crate graph easier to consume and package, not merely add an `xtask` and CI file.

## Confirmed good choices

- The move to R3F v9 requires React 19: its installed peer range is `react >=19 <19.3`; the React upgrade is therefore dependency-driven, not gratuitous.
- `Canvas frameloop="demand"` is the right idle-rendering configuration. Current R3F documentation says it renders on prop changes, and current drei documentation says `CameraControls` is compatible with demand mode.
- Geometry disposal in `RocketBody` is present. Three.js documentation requires disposal of unused `BufferGeometry` GPU resources.
- `get_vehicle_markers` is read-only and the frontend cancels stale marker responses, which preserves the render-layer command invariant.

## Monitoring checklist

- [x] Re-check the worktree after Claude committed the five v0.4 streams.
- [x] Run the complete `cargo xtask test` gate (Rust workspace tests, typecheck, 93 Vitest tests) and the production build.
- [ ] Inspect the final viewport visually at desktop and narrow width before calling the aesthetic work complete.
- [x] Reconcile the new crate/workspace/MCP changes against the no-network and single-dispatcher invariants.
