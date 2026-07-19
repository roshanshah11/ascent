# Claude Review Fixes Design

## Goal

Resolve every actionable item in `docs/CLAUDE_BUILD_REVIEW.md` while preserving Ascent's central invariants: deterministic simulation, one journaled mutation path, read-only rendering, and no runtime network access.

The work spans three bounded areas: frontend startup and visual quality, canonical command discovery, and MCP/workspace correctness.

## 1. Frontend startup and workbench finish

`ViewportR3F` will become a lazily loaded island. `App.tsx` will use `React.lazy` and a narrow `Suspense` boundary only when 3D is selected. The 2D workbench, command palette, and inspectors remain in the initial bundle. Production-build verification will compare the initial chunk with the current 1.18 MB baseline and require a separate viewport chunk.

The Design workspace will use named CSS-grid panel classes instead of its top-level inline flex layout. The viewport receives an explicit responsive height and panel treatment. Shared CSS will cover buttons, text inputs, focus-visible states, disabled states, panels, and narrow-screen behavior. Existing component semantics remain unchanged and no component framework is added.

Tests will cover the 2D/3D toggle and lazy fallback without loading WebGL in jsdom. The viewport's pure mesh and marker transforms remain independently tested.

## 2. Rust-owned command catalogue

Rust will own a `CommandGrammarEntry` catalogue containing each verb, usage string, and description. The parser, palette catalogue, and journal text format will share that definition rather than maintaining an independent TypeScript list.

`ascent-app` will expose the catalogue through a read-only Tauri command. The frontend will fetch it with the initial document data and pass it into `CommandPalette`. Failure to fetch the catalogue will surface an actionable error rather than silently using stale suggestions.

Rust tests will assert that the catalogue has unique verbs and exactly covers every parser variant. Frontend tests will consume injected catalogue data instead of importing a static mirror. A Rust test will also execute a console command, inspect the canonical journal entry, and verify replay.

## 3. Official RMCP stdio server

`ascent-mcp` will migrate from its hand-written JSON-RPC dispatcher to the official `rmcp` Rust SDK using only its server, macros, and stdio transport features. Tokio is an execution runtime, not authorization for network access; the binary will instantiate only stdin/stdout transport and will contain no HTTP client or listener.

The five public tools remain thin adapters over the existing seams:

- `get_document` reads state.
- `propose_commands` parses and dry-runs command lines.
- `apply_proposal` revalidates and atomically journals a batch.
- `run_study` starts work through a task service.
- `read_evidence` reads deterministic evidence.

The SDK owns initialization negotiation, tool discovery, schemas, request validation, and protocol error shapes. Study IDs use checked `u64` to `u32` conversion.

### Task lifecycle

Task-augmented `run_study` returns promptly with a task ID. A server-owned worker transitions the task through queued, running, and one terminal state: completed, failed, or cancelled. Task reads and cancellation are available while the stdio server continues processing requests. Completed results land through the same journaled study-results command used elsewhere.

If the selected RMCP release-candidate API does not yet expose the Tasks extension, Ascent will keep the five SDK-backed tools but omit Tasks from advertised capabilities and expose `run_study` synchronously. It will not emulate Tasks with an already-completed wrapper. This fallback is explicit, tested, and documented.

Tests will use an in-memory RMCP client/server transport where supported, plus a spawned stdio round trip for the binary. They will verify initialization, tool discovery, proposal/application staleness, checked IDs, and the real task-state transitions or the explicit no-Tasks fallback.

## 4. Workspace metadata and public surfaces

Every workspace crate will inherit the MIT license explicitly. Public crate roots will start with concise `//!` documentation identifying intended consumers, supported exports, and internal implementation modules. Existing curated `pub use` lists remain intact.

## Error handling

- Lazy viewport load failures render a scoped panel error and do not take down the workbench.
- Command-catalogue fetch errors are visible and disable grammar suggestions; named UI actions remain available.
- RMCP tool failures use SDK-standard tool errors with actionable messages.
- Numeric overflow is rejected as invalid input.
- Task cancellation is idempotent for queued/running tasks and cannot rewrite a completed result.

## Verification

Implementation proceeds test-first in small cycles. Final verification requires:

1. Focused frontend, command-catalogue, MCP, and metadata tests.
2. `cargo xtask test` with zero failures.
3. `npm run build` with the 3D stack separated from the initial JS chunk.
4. `git diff --check`.
5. Visual inspection of the Design workspace at desktop and narrow width, including keyboard focus and the lazy viewport state.
6. Confirmation that unrelated changes in `crates/ascent-sim/src/atmosphere.rs`, `crates/ascent-sim/src/lib.rs`, and `crates/ascent-sim/src/profile.rs` remain untouched.

## Non-goals

- No changes to simulation physics or current simulator work in progress.
- No new runtime network transport.
- No component framework or general UI rewrite.
- No changes to the journal grammar's external command text.
- No fake or immediately completed MCP task façade.
