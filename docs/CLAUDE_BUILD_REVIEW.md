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

**Evidence.** The current frontend suite passes (**88 tests**), and `cargo test -p ascent-app` passes. The new palette component test proves its mocked `onExec` callback is called for one line, but it does not prove an actual palette command reaches `console_exec` and becomes a journal entry. `ViewportR3F` has no direct rendered/component test; the new mesh-group tests cover only pure grouping math.

**Correction.** Add a focused Rust/Tauri integration test that calls `console_exec`, then inspects `session_journal` for the canonical line and verifies replay. For the viewport, add a lightweight Canvas-mocked component test covering CP/CG marker data and the 2D/3D toggle, while retaining pure mesh tests. Keep the full suite and production build as gates.

**Why it matters.** The acceptance criteria are command journaling, read-only rendering, and a live tree-derived viewport. Unit tests alone are not evidence for those boundaries.

## Confirmed good choices

- The move to R3F v9 requires React 19: its installed peer range is `react >=19 <19.3`; the React upgrade is therefore dependency-driven, not gratuitous.
- `Canvas frameloop="demand"` is the right idle-rendering configuration. Current R3F documentation says it renders on prop changes, and current drei documentation says `CameraControls` is compatible with demand mode.
- Geometry disposal in `RocketBody` is present. Three.js documentation requires disposal of unused `BufferGeometry` GPU resources.
- `get_vehicle_markers` is read-only and the frontend cancels stale marker responses, which preserves the render-layer command invariant.

## Monitoring checklist

- [ ] Re-check the worktree when each Claude stream finishes.
- [ ] Re-run frontend tests, the production build, and affected Rust tests after the final write burst.
- [ ] Inspect the final viewport visually at desktop and narrow width before calling the aesthetic work complete.
- [ ] Reconcile any new crate/workspace/MCP changes against the no-network and single-dispatcher invariants.
