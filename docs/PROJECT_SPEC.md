# ASCENT — Professional Aerospace Engineering Workbench

*Direction revised 2026-07-19. `PRODUCT_DIRECTION.md` is authoritative; `plan/2026-07-18-v1-roadmap.md` defines the capability sequence.*

## One-liner

Model aerospace vehicles, run validated and repeatable flight-dynamics studies, inspect uncertainty, and produce traceable evidence in a native workbench that engineers and AI agents operate through one audited command layer.

## Thesis

Serious open aerospace tools offer deep capability but often require specialists to assemble libraries, scripts, files, and visualization surfaces into a working campaign. Ascent occupies the missing workbench layer: Basilisk-grade evidence discipline (validation, provenance, and verification rigor — not comparable model depth), modular fidelity, managed studies, undo, visualization, and agent operation in one coherent local application.

The measuring user is a GNC or flight-dynamics engineer at a new-space or defense-tech startup. Ascent is built for craft, not for a sales wedge. It is not student software, competition-team software, a hobby product, or an OpenRocket replacement.

## Current technical slice

Ascent currently uses sounding-rocket-class vehicles as its first validated dynamics domain. The application provides:

1. Parametric vehicle modeling with a hierarchical model tree, mass properties, CP/CG, and explicit assumptions.
2. Deterministic point-mass and rigid-body simulation with named fidelity boundaries, event timelines, atmosphere inputs, recovery, and repeatable dispersions.
3. Managed studies whose inputs are hashed and whose stale state is explicit after model changes.
4. Requirements verification through versioned rule packs. IREC/DTEG is one optional example profile, not the default identity.
5. Evidence artifacts containing input identity, model versions, assumptions, convergence status, validation comparisons, and credibility limits.
6. A typed command spine shared by the GUI, console, CLI, and AI proposal/approval flow.

## Professional bar

- Validate physics against named analytical, tool, and flight-data baselines before making claims.
- Preserve bit-for-bit journal replay and deterministic seeded studies.
- Keep mutations behind the dispatcher; previews never silently alter canonical state.
- Make uncertainty, validity regimes, provenance, and stale results visible.
- Keep external data local, versioned, and hash-linked to derived evidence.
- Prefer modular interfaces that can grow toward environment, sensor, actuator, control-law, and GNC studies.

## Build direction

Follow `plan/2026-07-18-v1-roadmap.md`: deepen physics and validation, build professional data and geometry exchange, expose the audited command seam through MCP, produce deterministic trust artifacts, and make the workbench itself a serious engineering surface.
