# Ascent Professional Workbench UI Research

*Updated: 2026-07-19 | Scope: desktop engineering-workbench architecture, interaction, typography, and AI control surfaces*

## Executive conclusion

Ascent should stay a Tauri application and should not imitate a web dashboard. Tauri leaves the full React, CSS, SVG, Canvas, and WebGL rendering surface available in the operating system webview while the Rust process owns commands, simulation, files, and native integration. The correct product model is a dense, task-oriented engineering workbench: persistent model tree, central view, contextual properties, managed computation, journal/console, and explicit status.

The implemented visual system combines three proven ideas:

- Blender's stable window anatomy and task-specific workspaces.
- ParaView's visible data/analysis pipeline, contextual properties, and deliberate apply/update state.
- NASA Open MCT's domain-object tree and operational emphasis on planning, analysis, and telemetry-producing systems.

This is an Ascent-specific synthesis, not a visual clone of any one product.

## Findings and implementation consequences

### 1. Professional software is organized around durable work objects

Blender defines the application window through a persistent top bar, task-specific workspaces, editor areas and regions, and a status bar. Its 3D Viewport itself is composed of a header, toolbar, sidebar, and optional shelf. The important principle is not Blender's exact appearance; it is that tools and properties remain attached to stable regions around the primary work object. [Blender 5.0 Manual: Window System](https://docs.blender.org/manual/en/5.0/interface/window_system/introduction.html) and [3D Viewport](https://docs.blender.org/manual/en/5.0/editors/3dview/index.html).

Implementation consequence: Ascent uses a persistent activity rail and workspace context bar, a central vehicle view, a model explorer on the left, configuration properties on the right, a dock below, and a status line at the bottom.

### 2. The tree is the project, not secondary navigation

NASA describes Open MCT as a mission-control framework for planning, operations, and analysis of systems that produce telemetry. Its own glossary says that anything appearing in the left-hand tree is a domain object, and that composition describes the objects contained by another object. [NASA Open MCT repository](https://github.com/nasa/openmct#open-mct) and [Open MCT glossary](https://github.com/nasa/openmct#glossary).

Implementation consequence: Ascent's explorer contains vehicle parts, atmosphere, motor data, flight model, and saved studies in one project hierarchy. It is not a generic menu.

### 3. Computation needs visible pipeline and update state

ParaView presents scientific work as a source/filter pipeline with a Pipeline Browser, Properties panel, views and layouts, selection, representation controls, animation, and explicit pipeline updates. Its guide also treats GUI actions, Python scripting, filters, property changes, and pipeline updates as different surfaces over the same visualization process. [ParaView User's Guide](https://docs.paraview.org/en/latest/UsersGuide/introduction.html).

Implementation consequence: Ascent surfaces saved studies and job state as managed pipeline objects. Result freshness is visible globally. AI and manual changes are previewed before they are applied, with stale-study impact called out.

### 4. AI belongs inside the command and evidence model

Ascent's existing agent contract already defines the safe pattern: read state, propose a command batch, inspect the dry-run verdict and exact diff, approve, then apply atomically. The agent does not get a private mutation path. See [COPILOT_INTERFACE.md](./COPILOT_INTERFACE.md).

Implementation consequence: the dock includes an AI Command Copilot connected to `propose_commands` and `apply_proposal`. It shows canonical commands, validation errors, state-change count, and which studies would become stale. It does not claim that an embedded cloud model exists when the product is intentionally local and deterministic.

### 5. Tauri does not constrain the visual design

Tauri's frontend is an ordinary web frontend running in the operating system webview, with JavaScript-to-Rust communication through `invoke`. Tauri explicitly supports virtually any frontend framework. [Tauri v2: What is Tauri?](https://v2.tauri.app/start/) and [Core JavaScript API](https://v2.tauri.app/reference/javascript/api/namespacecore/).

Implementation consequence: the app can use React, responsive CSS Grid, SVG engineering drawings, React Three Fiber/Three.js, custom charts, split panes, and future GPU-backed visualization. Tauri is the native shell and trust boundary, not a UI template system.

### 6. Dense controls still need predictable accessibility

W3C's Authoring Practices defines keyboard and focus behavior for toolbars, tabs, spinbuttons, disclosure controls, and other composite widgets. [WAI-ARIA Authoring Practices Guide](https://www.w3.org/WAI/ARIA/apg/patterns/).

Implementation consequence: workspaces remain real tabs with selected/disabled state, the viewport control group is a toolbar, focus is visible, controls retain semantic labels, and keyboard shortcuts do not fire while editing form fields.

## Visual system

- Typography: native desktop system UI stack for labels and commands; SF Mono-compatible stack only for measurements, hashes, commands, and status data.
- Density: 23-42 px chrome bands, 25-30 px tree/property rows, 10-12 px labels, and 1 px separators.
- Palette: neutral graphite surfaces; oxide orange as the single product accent; blue, green, yellow, and red reserved for engineering/status semantics.
- Shape: 2-4 px radii for controls only; no card grid, oversized radius, glow, glass, gradient text, or sci-fi HUD decoration.
- Motion: direct hover/press/focus feedback; no perpetual animation in an analytical workbench.
- Canvas: real design values and CP/CG markers, explicit orthographic/perspective state, measurements, and a controlled drawing title block.

## Skill and source audit

Targeted searches for desktop engineering workbench UI, scientific-visualization UX, mission-control displays, and technical typography did not produce an installable specialist skill with a strong enough combination of source authority, adoption, and relevance. No additional skill was installed. The implementation instead uses the already-installed frontend/design skills plus official primary sources above.

## Research limitations

The requested Exa and Firecrawl research backends were not available in this session. Research therefore used Context7's indexed official Blender, ParaView, and Tauri documentation plus direct official NASA, ParaView, and W3C sources. This is strong for product anatomy and interaction patterns, but it is not a broad survey of commercial aerospace software screenshots or proprietary design systems.
