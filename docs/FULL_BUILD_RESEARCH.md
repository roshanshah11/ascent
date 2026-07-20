# Ascent Professional Build Research

*Reframed 2026-07-19 after the project-direction correction. This document records technical foundations and build implications. Product identity is defined by `PRODUCT_DIRECTION.md`; release sequencing is defined by `plan/2026-07-18-v1-roadmap.md`.*

## Research question

What must Ascent implement for a GNC or flight-dynamics engineer at a new-space or defense-tech startup to regard it as serious aerospace software?

The answer is not a beginner-oriented interface or a market wedge. It is a coherent workbench with validated physics, modular fidelity, repeatable studies, uncertainty, traceable evidence, professional interoperability, and an audited automation surface.

## Reference bar

- **Basilisk** establishes the architecture bar: modular simulation, configurable fidelity, deterministic Monte Carlo, GN&C workflows, hardware-in-the-loop paths, and a separate high-quality visualizer.
- **Nyx** establishes a credibility pattern: validate a newer implementation against an accepted baseline such as GMAT and publish the comparison.
- **GMAT** establishes professional expectations for mission analysis, navigation, optimization, scripting, and inspectable configuration.
- **RocketPy** establishes the immediate rocketry-domain physics bar: six-degree-of-freedom dynamics, variable-mass effects, weather, multi-stage events, recovery, Monte Carlo, and published validation against flight data.
- **OpenRocket** is a useful compatibility and regression baseline for sounding-rocket-class cases. Its product scope and architecture are not Ascent's ambition bar.

The empty seat is **Basilisk-grade rigor inside a real workbench that an AI agent can operate**.

## Capability requirements

### 1. Document and command architecture

The project document must be the source of truth for vehicle, environment, studies, requirements profiles, imported data, and evidence references. Every mutation crosses one typed dispatcher shared by GUI, console, CLI, MCP, and future extensions.

Required invariants:

- journal replay reconstructs identical canonical state;
- previews never mutate or journal canonical state;
- a continuous gesture emits exactly one command on release;
- undo and redo operate at engineering-action boundaries;
- derived studies become visibly stale when hashed inputs change;
- agent proposals expose commands, validation errors, diffs, and stale impact before approval.

### 2. Physics and fidelity

Keep multiple fidelity tiers explicit rather than presenting one solver as universally authoritative:

- fast point-mass dynamics for interaction and broad sweeps;
- rigid-body six-degree-of-freedom dynamics for attitude-coupled flight;
- correct variable-mass and staging/event treatment;
- imported atmospheres, wind ensembles, recovery phases, and landing dispersion;
- modular hooks for sensors, actuators, control laws, and later GNC studies;
- aero coefficient tables with source, interpolation rules, and validity metadata;
- deterministic seeded Monte Carlo and sensitivity studies.

Every output must state the model version, timestep/integrator configuration, environment source, assumptions, and known validity limits.

### 3. Validation and uncertainty

Validation is a maintained library, not a one-time demo comparison. It should include:

- analytic invariants and conservation checks;
- timestep/order convergence tests;
- cross-engine fixtures against RocketPy, OpenRocket where applicable, and later GMAT or Basilisk for overlapping regimes;
- public or user-imported flight datasets with predicted-versus-observed metrics;
- deterministic rerun equality;
- uncertainty and sensitivity results linked to the exact sampled inputs.

Tool disagreement is a result to inspect, not an inconvenience to hide.

### 4. Workbench UX

The native workbench should use stable professional anatomy:

- project/model tree on the left;
- central geometry, trajectory, plot, or study view;
- contextual properties and units on the right;
- managed jobs, console, journal, and agent proposals in a dock;
- persistent solver, freshness, coordinate-frame, and selection state;
- keyboard-first commands and reusable workspace layouts.

Dense presentation is appropriate when hierarchy, typography, spacing, and focus behavior stay precise. Avoid dashboard cards, decorative HUD styling, and hidden computation state.

### 5. Evidence and interoperability

Each decision-relevant result should be exportable with:

- canonical inputs and hashes;
- solver and model versions;
- fidelity regime and assumptions;
- uncertainty and convergence status;
- validation comparisons;
- requirement outcomes and source identifiers;
- journal provenance.

Interop should prioritize professional data movement: documented project schemas, CSV/Parquet/HDF5 where appropriate, atmosphere and telemetry imports, aero database imports, geometry exchange, Python access, and versioned CLI/MCP contracts.

## Requirements profiles

Mission, range, organization, and regulatory constraints belong in versioned data packs. The bundled IREC 2026/DTEG pack remains a useful example and regression fixture because it provides concrete, sourced thresholds for rail exit, stability, geometry, and reporting. It must be presented as an optional active profile, not as Ascent's audience, default identity, or roadmap justification.

Passing a profile is an engineering aid, never a claim of official approval.

## AI-native operation

Ascent does not need an embedded model to be AI-native. Its durable advantage is that an external agent can:

1. read a structured document;
2. propose typed command batches;
3. receive a deterministic dry-run verdict and exact diff;
4. surface invalidations and evidence requirements;
5. apply only after explicit approval;
6. run studies and read structured evidence through the same public seam.

The LLM proposes. Deterministic software mutates, solves, verifies, and records.

## Build priorities

1. Preserve command/journal and determinism invariants.
2. Complete and validate six-degree-of-freedom, staging, environment, recovery, and dispersions.
3. Treat studies, requirements, validation cases, and evidence as first-class project objects.
4. Expose the command/evidence seam through CLI and MCP with agent evals.
5. Add flight-data reconciliation and maintained baseline comparisons.
6. Expand modularly toward sensors, actuators, control laws, optimization, and GNC campaigns.
7. Ship signed, documented, reproducible desktop builds with stable file and automation contracts.

## Primary technical references

- [Basilisk documentation](https://avslab.github.io/basilisk/)
- [Nyx](https://github.com/nyx-space/nyx)
- [NASA GMAT](https://github.com/nasa/GMAT)
- [RocketPy](https://github.com/RocketPy-Team/RocketPy)
- [OpenRocket technical documentation](https://openrocket.sourceforge.net/techdoc.pdf)
- [1976 U.S. Standard Atmosphere](https://ntrs.nasa.gov/citations/19770009539)
- [Tauri v2 documentation](https://v2.tauri.app/)

Historical IREC/DTEG source documents remain cited by the versioned rule-pack data and its tests, where their provenance belongs.
