# From Tool to Software — Research: how Ascent becomes a real OpenRocket competitor

*2026-07-17. Deep-research synthesis (web, cited). Feeds `docs/plan/2026-07-17-v02-professionalization.md`. Companion to `IDEA_SPEC.md` and `FULL_BUILD_RESEARCH.md`.*

## The honest diagnosis

Ascent v0.1 is a **validated demo**, not software. What it has that most demos don't: deterministic provenance, external validation (1.4% vs OpenRocket), a cited rule engine, 98 tests. What it lacks that all real engineering software has: a document model (open/save/undo), a real vehicle model (one hardcoded airframe now), 3D/6-DOF physics, persistence, extensibility, and a trust framework beyond one fixture. The gap between the two is not "more features" — it is **five specific architectural layers**, each with an industry-standard shape.

## 1. Architecture of professional CAD/sim software

The canonical structure, consistent across [WELSIM's architecture writeup](https://getwelsim.medium.com/design-and-architecture-of-modern-general-purpose-engineering-simulation-software-c923809a66cf), [geometric-kernel practice](https://en.wikipedia.org/wiki/Geometric_modeling_kernel), and open CAD projects:

- **Strict layering: Core → Data/Document → Interaction → UI.** Core holds math, units, geometry abstractions; the Data layer holds the parametric model, feature graph, and document/session services; UI never touches solvers directly. Ascent already half-does this (run-state machine and playback are pure modules; only `ipc.ts` touches the bridge) — the missing layer is **Document** (the thing between "a Design struct" and "a file the user owns").
- **Solvers are swappable numerical engines**, decoupled from both the physics model and the UI — same pattern as [modular research solvers](https://arxiv.org/pdf/2603.14040). This is exactly what the cross-validation feature (IDEA_SPEC #3) needs: `SimEngine` as a trait, native RK4 as implementation #1, RocketPy/OpenRocket bridges as #2/#3.
- **Extensibility via a plugin manager with factory-registered components** — the [mbsolve pattern](https://arxiv.org/pdf/2005.05412): `create_instance(name)` looks up a registered subclass. Motors, drag models, rule packs, and engines should all load this way instead of `include_str!` constants.
- **Cautionary tale:** CADDS5 died partly from *lack of modularity* — a frozen monolith nobody could replatform ([kernel wars history](https://demystifyingplm.ghost.io/kernel-wars/)). SolidWorks won 1993–2000 partly by **licensing components instead of building everything** (Parasolid kernel licensed in, per [engineering.com's founding history](https://www.engineering.com/founding-and-developing-solidworks-and-onshape/)) — the "wrap, don't rebuild" principle Ascent's fidelity ladder already commits to.

## 2. What makes a desktop app feel "professional" (the table stakes layer)

From [command-pattern practice](https://www.esveo.com/en/blog/undo-redo-and-the-command-pattern/) and crash-recovery patterns ([CodeProject autosave architecture](https://www.codeproject.com/articles/324/autosave-and-crash-recovery)):

1. **Document model as the single mutation point** — every edit is a Command object with execute/unexecute on past/future stacks. Undo/redo falls out. Ascent's `EDIT` dispatch is already command-shaped; it just doesn't retain the inverse.
2. **Plain-text project file** (the .ork lesson from IDEA_SPEC): versioned schema, git-diffable, with the run records embedded or side-car. This is simultaneously a feature (team-native) and the moat OpenRocket can't easily copy (their format is zipped XML).
3. **Canonical internal units, convert at presentation boundary** — [unit-aware data model pattern](https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/9405284). Ascent is SI-internal already; the UI needs a display-units layer (ft/m toggle — rocketry is aggressively mixed-unit: motors metric, apogees imperial).
4. **Autosave to a temp location + recovery manager on launch** — MS-Office pattern; never overwrite the user's file.
5. **Multi-design workspace** — more than one rocket open, compare runs across designs.

## 3. Trust: the V&V framework is the moat, not a chore

- **NASA-STD-7009B (2024)** scores simulation credibility 0–4 across [Capability and Results assessment categories](https://standards.nasa.gov/sites/default/files/standards/NASA/w/CHANGE-1/1/nasa_std_7009a_change_1.pdf), and requires: declared limits of operation, recognizing unrealistic results, uncertainty characterization, and decision-maker-readable reporting. **Ascent's evidence drawer is a baby version of this.** Formalizing it as "the 7009-inspired credibility scorecard" turns a demo feature into a standards-anchored differentiator no hobby tool has.
- **ASME V&V 20** quantifies validation uncertainty at specific validation points — and explicitly says extrapolation beyond validated points is engineering judgment ([overview](https://www.osti.gov/servlets/purl/1368927)). Translation for Ascent: every prediction should carry *"inside/outside validated regime"* metadata (subsonic Barrowman validated; transonic = flagged).
- **The ESRD critique**: [standards compliance ≠ decision-grade reliability](https://www.esrd.com/trustworthiness-in-simulation-credibility-or-decision-grade-reliability/) — trust is outcome-based accuracy on the quantity you care about. Ascent's answer is the validation library: accumulate real flight data (teams publish it — GTXR, EPFL, Notre Dame) and show per-quantity error distributions, exactly how [RocketPy earned ~1% apogee credibility](https://ascelibrary.org/doi/10.1061/(ASCE)AS.1943-5525.0001331).

## 4. Physics roadmap (what "not a toy" requires, in order)

What users actually complain OpenRocket lacks ([Stanford SSI list](https://github.com/stanford-ssi/openrockoon), [forum threads](https://www.rocketryforum.com/threads/openrocket-limits.142669/)): CFD coefficient import, Monte Carlo, better atmosphere models, spin/Magnus, engine altitude compensation, transonic accuracy.

- **3-DOF with wind → true 6-DOF**: [RocketPy's architecture](https://github.com/RocketPy-Team/RocketPy) is the reference — modular SRP classes, variable-mass 6-DOF, weather-model ingestion (forecast/reanalysis ensembles), validated vs. multiple university flights. Its peer-reviewed papers are a free blueprint.
- **Monte Carlo dispersion** is the highest-leverage physics feature: it's what range-safety submissions need, [GTXR ran 3,000-sample dispersions incl. failure modes](https://arxiv.org/pdf/2411.00807), and OpenRocket doesn't have it natively. It requires only the existing 1-DOF/3-DOF core + parameter distributions — **buildable before 6-DOF**.
- **Aero fidelity ladder** stays as FULL_BUILD_RESEARCH decided: imported Mach/AoA coefficient tables first (the safe boundary), transonic via RASAero-style methods later, CFD as export-a-case only.

## 5. Stack verdict: Rust + Tauri holds; 3D needs one decision

- **Tauri v2 at scale is proven**; the one hard rule is coarse IPC — [batch, never hundreds of small invokes](https://www.plutenium.com/blog/building-desktop-apps-with-rust-and-tauri) — which Ascent already obeys. Keep it.
- **3D rendering: [wgpu](https://wgpu.rs/)** (native WebGPU, runs Metal/Vulkan/DX12 and in-webview). For rocket geometry — bodies of revolution + fin plates — **you do not need a B-rep CAD kernel**. Generate meshes procedurally from the parametric model.
- **If/when real CAD ops are needed** (custom fin cans, 3D-printed parts): [truck](https://github.com/ricosjp/truck) is the all-Rust+wgpu NURBS option (immature booleans), [opencascade-rs](https://github.com/bschwind/opencascade-rs) wraps the industrial kernel (C++ dependency), and [Fornjot is dead](https://www.fornjot.app/) — ruled out. Decision deferred until a user needs it; bodies-of-revolution cover the market for years.

## 6. Company-shape lessons

- **[SolidWorks](https://www.engineering.com/founding-and-developing-solidworks-and-onshape/)**: won by riding a platform shift (Windows) + licensing components + community. Ascent's platform shift is AI-native + Rust-native performance.
- **[Onshape](https://www.goengineer.com/blog/onshape-vs-solidworks-the-complete-story)**: born from watching users suffer (install/versioning/data management — literally the .ork pain transposed); shipped **every 3 weeks** vs. multi-year CAD cycles; app store made it a platform → $470M exit. Cadence itself is a differentiator against OpenRocket's release pace.
- **[Ansys](https://www.ansys.com/company-information/the-ansys-story)**: bootstrapped from consulting, 50-year compounding on trust + R&D >20% of revenue. Trust compounds; features don't.
- **[KiCad](https://www.kicad.org/about/kicad/)**: the open-source-to-professional path runs through institutional patrons (CERN), foundation governance, grants (NLnet €50K), corporate sponsors — "clunky hobby tool" → >15% of new commercial board orders. FreeCAD is the anti-pattern: single-maintainer dependence. Relevant if Ascent goes open-core: court the *teams* and their sponsors as the institutions.
- **Common thread**: every one obsessed over a specific user's specific pain and shipped relentlessly. None started as a platform; all started as a sharp tool that grew layers.

## The five layers, named (this is the plan's skeleton)

1. **Document layer** — project file format, open/save, undo/redo commands, autosave/recovery, multi-design workspace.
2. **Registry layer** — motors/drag models/rule packs/engines as registered plugins, ThrustCurve.org full motor DB, .eng/.ork import (the A6 corpus is staged for exactly this).
3. **Physics ladder** — wind + 3-DOF, Monte Carlo dispersion, imported aero tables, then 6-DOF (RocketPy-informed), transonic methods.
4. **Trust framework** — NASA-7009-inspired credibility scorecard, validated-regime flags on every number, real-flight validation library, cross-engine spread (RocketPy bridge first: MIT-licensed, scriptable, validated).
5. **Presentation** — the post-Jul-23 UI redesign (already scheduled, brainstorming + design skills), 3D viewport via wgpu procedural meshes, display-units system.

Sequencing rule (from every case study): each layer ships as small user-visible slices on a fast cadence, never as a big-bang rewrite. Layer 1 + 2 make it *software*; layer 3 + 4 make it a *competitor*; layer 5 makes it *feel* like a product.
