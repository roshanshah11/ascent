# The CAE Paradigm — how Fluent/STAR-CCM+/NX/Blender are actually built

*2026-07-18. Web research synthesis (cited). Feeds `docs/plan/2026-07-18-v03-cae-skeleton.md`. Companion to `PROFESSIONAL_SOFTWARE_RESEARCH.md` (which covers layering, document model, V&V — this doc covers the application paradigm itself and the AI-native opening).*

## Mission this research serves

**Ascent = the AI-native aerospace simulation workbench.** Fluent/NX-class structure; differentiators no incumbent has: (1) an AI that operates the software as a first-class client of the command layer, and (2) determinism + provenance + credibility scoring as core architecture. Rocketry is the first domain pack, not the identity.

## The one unifying pattern

Across every professional tool studied, the same inversion appears: **the GUI is not the program. The program is a command/data core, and the GUI is one of several clients.**

- **Blender**: everything is a *data-block* (mesh, object, scene, screen — typed, ref-counted, name-unique, with pointer restrictions that keep the database coherent), and every action is an *operator* — executable given only a Context, with all inputs stored as properties, which is exactly what makes operators "re-usable, stackable into macros or history, savable in files, or used by scripting" ([Blender architecture overview](https://archive.blender.org/wiki/2015/index.php/Dev:Source/Architecture/Overview/), [data-blocks manual](https://docs.blender.org/manual/en/latest/files/data_blocks.html)). The UI button and the Python call route to the *same* operator.
- **Fluent**: the TUI/journal layer mirrors the full capability surface; Ansys explicitly recommends writing journals in TUI commands *rather than* recording the GUI, because the GUI changes and the command layer is the stable contract ([Sheffield HPC journal guide](https://docs.hpc.shef.ac.uk/en/latest/referenceinfo/ANSYS/fluent/writing-fluent-journal-files.html)). PyFluent then wraps that same surface as Python settings objects over gRPC — the solver is a server, every client (GUI, script, cluster scheduler) speaks commands ([PyFluent](https://github.com/ansys/pyfluent)).
- **NX**: journaling records any interactive session as replayable code through the Common API — same API surface in every language, record/playback in every seat ([NX Journaling](https://www.nxjournaling.com/content/beginning-journaling-using-nx-journal)). Siemens' own guidance: treat journal code as production software.
- **STAR-CCM+**: one integrated environment where the **simulation object tree is the UI** — model nodes in the tree *are* the active physics; Java macros walk the same tree the mouse does ([Siemens STAR-CCM+](https://www.siemens.com/en-us/products/simcenter/fluids-thermal-simulation/star-ccm/), [Java macro KB](https://community.sw.siemens.com/s/article/Simcenter-STAR-CCM-Java-Knowledge-Base-Table-of-contents)).

**Implication for Ascent:** the v0.3 keystone is a serializable command dispatcher + journal in the Rust core, with the model tree owned by Rust and the React UI demoted to a thin client. The AI copilot then needs *zero special machinery* — it is just another client of the same audited command stream. This is the architectural move that makes "AI-native" real instead of a chat sidebar.

## The CAE application skeleton (what the paradigm consists of)

1. **Model/object tree** — typed part hierarchy, single source of truth; physics, mesh, and mass properties all derive from it (STAR-CCM+ tree, Blender data-blocks).
2. **Study/case system** — a design owns multiple configured analyses (STAR-CCM+ "stages": multiple configurations, each with its own physics, inside one model).
3. **Managed jobs** — solves run as background jobs with progress/residual monitoring, batch/headless execution as a first-class mode (Fluent `-g` batch + checkpoint sentinels on clusters).
4. **Post-processing environment** — results analysis is its own workspace, not a footer.
5. **Journal/console/scripting** — the automation layer mirrors the full tree; recording, replay, headless.
6. **Data management** — versioned, provenance-stamped artifacts (Teamcenter-shaped; Ascent's hash-stamped runs already lean this way).

## The AI-native opening (2026 state of the art)

The field is converging on three approaches: agentic orchestration of simulation pipelines ([SimScale Engineering AI agents + Workflows, May 2026](https://www.simtool.com/article/simscale-launches-engineering-ai-agents-and-open-workflows-platform-for-autonomous-simulation-orches/)), physics-aware geometry generation ([Neural Concept's Design Copilot, $100M Series C](https://www.engineering.com/neural-concept-launches-ai-design-copilot-for-engineers/)), and copilots bolted onto existing CAD ([MecAgent](https://mecagent.com/), Leo AI, Ansys Engineering Copilot). The open research gap is **validation**: generative CAD is scored on geometric similarity, not functional validity — motivating "physics-in-the-loop" hybrid agentic architectures ([arXiv 2605.19717](https://arxiv.org/pdf/2605.19717)).

**Ascent's position:** nobody in that list has a deterministic, provenance-stamped, credibility-scored core for the AI to operate. The incumbents bolt AI onto GUI-first architecture; the startups generate geometry without evidence. Ascent inverts both: command-layer-first architecture (so the AI is a native operator) + evidence-first numbers (so the AI's actions are auditable and its claims cite checked-in validation). That combination is the moat, and both halves already exist in embryo (command-based undo, evidence drawer, credibility scorecard).

## Design rules extracted for v0.3

1. Every mutation is a named, serializable command dispatched through one core dispatcher. No mutation path bypasses it — not the GUI, not the console, not the AI.
2. The journal is the product's memory: append-only, replayable, and replay must reproduce state byte-identically (determinism discipline extends to the command layer).
3. The model tree lives in Rust. React renders state snapshots and sends commands; it owns nothing.
4. Physics, mesh, mass properties, and reports all derive from the tree — one rocket, one source of truth (kills both provisional geometries: `planar_vehicle_for` and the 12-caliber mesh).
5. Studies are documents: configuration + engine + seed + results reference, saved in the project file.
6. Headless-first: everything the GUI does must run from the CLI replaying a journal. This is simultaneously the CI story, the cluster story, and the AI story.
7. GUI stays disposable until the dedicated design pass; the workbench *layout* (tree/viewport/properties/console/jobs) lands as skeleton.
