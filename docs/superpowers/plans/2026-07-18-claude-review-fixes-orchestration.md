# Plan-Orchestrate Result

> Warning: could not detect ECC install; defaulting to legacy form. If you use the plugin install, edit the prefixes manually.

**Plan**: `docs/superpowers/plans/2026-07-18-claude-review-fixes.md`
**Lang**: `unknown`
**ECC mode**: `legacy`
**Steps**: 5
**Scope**: `all`

## Steps overview

| # | Title | Tags | Chain |
|---|---|---|---|
| 1 | Lazy and visually finish the R3F workbench | impl, test, build, review | `tdd-guide,e2e-runner,build-error-resolver,code-reviewer` |
| 2 | Make command discovery Rust-owned | impl, test, review | `tdd-guide,e2e-runner,code-reviewer` |
| 3 | Migrate ascent-mcp to official RMCP stdio | impl, migration, lookup, test | `architect,tdd-guide,code-reviewer` |
| 4 | Complete workspace metadata and crate contracts | impl, docs, build | `tdd-guide,doc-updater,build-error-resolver,code-reviewer` |
| 5 | Integration, visual QA, and final review | test, build, review | `tdd-guide,e2e-runner,build-error-resolver,code-reviewer` |

---

## Step 1 — Lazy and visually finish the R3F workbench

**Intent**: Split the R3F island from startup, finish responsive Design-workspace layout, and verify the result visually and in production output.
**Tags**: impl, test, build, review
**Chain rationale**: TDD and E2E establish behavior; the build resolver validates chunking; code review closes the implementation chain.

```bash
/orchestrate custom "tdd-guide,e2e-runner,build-error-resolver,code-reviewer" "[Plan: docs/superpowers/plans/2026-07-18-claude-review-fixes.md#step-1] Lazy-load ViewportR3F behind Suspense, replace the Design flex shell with named responsive grid panels, and add shared mission-control controls/focus styling; Acceptance: focused App tests pass; production build emits a separate viewport chunk; desktop and narrow-width viewport states are visually coherent"
```

## Step 2 — Make command discovery Rust-owned

**Intent**: Replace the TypeScript grammar mirror with a read-only Rust catalogue and prove palette dispatch reaches the canonical journal path.
**Tags**: impl, test, review
**Chain rationale**: TDD owns the cross-language contract, E2E checks the IPC path, and code review gates the final API surface.

```bash
/orchestrate custom "tdd-guide,e2e-runner,code-reviewer" "[Plan: docs/superpowers/plans/2026-07-18-claude-review-fixes.md#step-2] Add a unique Rust CommandGrammarEntry catalogue, expose it through read-only Tauri IPC, inject it into CommandPalette, and delete the static TypeScript mirror; Acceptance: catalogue covers every parser verb; palette tests use injected entries; console dispatch journals and replays canonical bytes"
```

## Step 3 — Migrate ascent-mcp to official RMCP stdio

**Intent**: Replace hand-written protocol framing with RMCP 2.2 while preserving the five existing tools, offline operation, and truthful study execution semantics.
**Tags**: impl, migration, lookup, test
**Chain rationale**: Architecture resolves the SDK and Tasks boundary, TDD implements the transport and tools, and code review closes the protocol migration.

```bash
/orchestrate custom "architect,tdd-guide,code-reviewer" "[Plan: docs/superpowers/plans/2026-07-18-claude-review-fixes.md#step-3] Migrate ascent-mcp to RMCP 2.2 using server macros and stdio-only transport, keep the five tool names over existing seams, validate study IDs, and advertise Tasks only if the SDK supports a real lifecycle; Acceptance: SDK client/server and stdio tests pass; no HTTP feature or listener exists; proposal staleness remains correct"
```

## Step 4 — Complete workspace metadata and crate contracts

**Intent**: Make license inheritance complete and document every public crate surface without widening exports.
**Tags**: impl, docs, build
**Chain rationale**: TDD establishes the metadata gate, doc-updater writes the contracts, build resolution verifies Cargo metadata/docs, and code review closes the diff.

```bash
/orchestrate custom "tdd-guide,doc-updater,build-error-resolver,code-reviewer" "[Plan: docs/superpowers/plans/2026-07-18-claude-review-fixes.md#step-4] Add workspace-license inheritance to every missing crate and concise crate-level public-surface docs while preserving curated exports; Acceptance: Cargo metadata reports MIT for every workspace package; cargo doc succeeds without warnings; no public exports are added"
```

## Step 5 — Integration, visual QA, and final review

**Intent**: Run the complete release gate, inspect UI behavior and bundle output, and resolve final cross-task findings without touching concurrent simulator work.
**Tags**: test, build, review
**Chain rationale**: E2E and build agents verify runtime and production output; code review provides the final whole-plan gate.

```bash
/orchestrate custom "tdd-guide,e2e-runner,build-error-resolver,code-reviewer" "[Plan: docs/superpowers/plans/2026-07-18-claude-review-fixes.md#step-5] Verify and review the combined Claude-review fixes against the approved spec; Acceptance: cargo xtask test and npm build pass; initial JS excludes the R3F stack; desktop/narrow visual QA passes; git diff check is clean and concurrent simulator/STL/structural/eval edits are preserved"
```

## Batch execution

```bash
/orchestrate custom "tdd-guide,e2e-runner,build-error-resolver,code-reviewer" "[Plan: docs/superpowers/plans/2026-07-18-claude-review-fixes.md#step-1] Lazy-load ViewportR3F behind Suspense, replace the Design flex shell with named responsive grid panels, and add shared mission-control controls/focus styling; Acceptance: focused App tests pass; production build emits a separate viewport chunk; desktop and narrow-width viewport states are visually coherent"
/orchestrate custom "tdd-guide,e2e-runner,code-reviewer" "[Plan: docs/superpowers/plans/2026-07-18-claude-review-fixes.md#step-2] Add a unique Rust CommandGrammarEntry catalogue, expose it through read-only Tauri IPC, inject it into CommandPalette, and delete the static TypeScript mirror; Acceptance: catalogue covers every parser verb; palette tests use injected entries; console dispatch journals and replays canonical bytes"
/orchestrate custom "architect,tdd-guide,code-reviewer" "[Plan: docs/superpowers/plans/2026-07-18-claude-review-fixes.md#step-3] Migrate ascent-mcp to RMCP 2.2 using server macros and stdio-only transport, keep the five tool names over existing seams, validate study IDs, and advertise Tasks only if the SDK supports a real lifecycle; Acceptance: SDK client/server and stdio tests pass; no HTTP feature or listener exists; proposal staleness remains correct"
/orchestrate custom "tdd-guide,doc-updater,build-error-resolver,code-reviewer" "[Plan: docs/superpowers/plans/2026-07-18-claude-review-fixes.md#step-4] Add workspace-license inheritance to every missing crate and concise crate-level public-surface docs while preserving curated exports; Acceptance: Cargo metadata reports MIT for every workspace package; cargo doc succeeds without warnings; no public exports are added"
/orchestrate custom "tdd-guide,e2e-runner,build-error-resolver,code-reviewer" "[Plan: docs/superpowers/plans/2026-07-18-claude-review-fixes.md#step-5] Verify and review the combined Claude-review fixes against the approved spec; Acceptance: cargo xtask test and npm build pass; initial JS excludes the R3F stack; desktop/narrow visual QA passes; git diff check is clean and concurrent simulator/STL/structural/eval edits are preserved"
```
