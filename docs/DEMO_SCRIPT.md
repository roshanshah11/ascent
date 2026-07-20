# 90-second Ascent professional workbench demo

This demo is judged by the standard in `PRODUCT_DIRECTION.md`: show a GNC or flight-dynamics engineer a serious, reproducible analysis workflow. It does not present Ascent as competition, student, or hobby software.

| Time | Screen | Roshan says |
|---:|---|---|
| 0–10 s | Open a sounding-rocket vehicle campaign. The model tree, vehicle, CP/CG, atmosphere, solver configuration, and saved studies are visible. | “Ascent is a local aerospace engineering workbench. The model, environment, studies, and evidence live in one inspectable project.” |
| 10–23 s | Select a fin or mass-property parameter. Show units, provenance, and live stability consequences in the viewport. | “The geometry is not decoration. It is a view of the same model consumed by the solvers, with explicit units and mass properties.” |
| 23–36 s | Open a study and run a deterministic simulation or dispersion. Show seed, input hash, solver/fidelity choice, and job state. | “A study records the exact inputs, model version, fidelity, and seed. Re-running the same study produces the same result.” |
| 36–51 s | Inspect trajectories, event timeline, uncertainty envelope, and a comparison against another engine or validation case. | “Results are managed engineering objects. Uncertainty and disagreement with a baseline stay visible instead of collapsing into one authoritative-looking number.” |
| 51–65 s | Open Verification. Show active requirements and structural margins with values, thresholds, and source identifiers. | “Requirements are versioned data. This profile can represent mission, range, or organization-specific constraints without hard-coding them into the product.” |
| 65–80 s | Ask the Command Copilot for a design change. Show the proposed canonical commands, diff, stale-study impact, and approval control. | “The agent has no private write path. It proposes the same audited commands as the GUI, and I see the exact impact before approval.” |
| 80–90 s | Open Evidence and show hashes, assumptions, convergence, validation status, and journal history. | “The deliverable is not just a plot. It is a reproducible claim with its inputs, assumptions, validation, and operator history attached.” |

## Backup-video shots

1. Model tree and vehicle viewport with CP/CG and units.
2. Parameter manipulation with read-only preview, followed by one journaled commit.
3. Seeded study launch and visible job/freshness state.
4. Ensemble results, timeline, and baseline comparison.
5. Requirements verification with traceable sources.
6. Agent proposal, command diff, stale-study warning, and approval.
7. Evidence view with input hash, fidelity, convergence, and journal.

## Fallback narration

“Ascent keeps the vehicle, environment, studies, requirements, and evidence in one local engineering document. Every mutation crosses one journaled command layer, including agent proposals. Every result retains its input identity, fidelity, uncertainty, and validation context, so the workbench can support engineering judgment rather than merely producing a plausible plot.”
