# Ascent Product Direction

*Authoritative project context. Revised 2026-07-19 after the 2026-07-18 direction decision.*

## What Ascent is

Ascent is a local-first aerospace engineering workbench for modeling, simulation, uncertainty analysis, verification, and evidence. It aims for the evidence and workflow discipline expected from Basilisk, Nyx, RocketPy, and GMAT workflows — not their model or physics depth — inside a coherent native workbench that both engineers and AI agents can operate through the same audited command layer.

**North star:** Basilisk-grade evidence discipline — comparable verification, provenance, and validation rigor, not comparable model depth — inside a real workbench that an AI agent can operate.

The measuring user is a GNC or flight-dynamics engineer at a new-space or defense-tech startup. A feature belongs when that engineer would respect its numerical honesty, architecture, reproducibility, and usefulness.

## What Ascent is not

- It is not competition-team, collegiate, student, education, certification, or hobbyist software.
- It is not an OpenRocket replacement. OpenRocket is a compatibility target and one validation baseline, not the ambition bar.
- It is not presently a commercial product. There are no customers, sales motion, market wedge, pricing strategy, or TAM to optimize.
- It is not an AI chat wrapper. AI operates through typed proposals, explicit approval, deterministic commands, and evidence-producing solvers.

## Engineering priorities

1. Validated physics with explicit fidelity regimes and named baselines.
2. Bit-for-bit repeatability, uncertainty quantification, provenance, and immutable evidence.
3. Modular dynamics, environment, vehicle, sensor, actuator, and control-law interfaces.
4. Professional workflows: model tree, study management, requirements verification, results comparison, scripting, reports, and data reconciliation.
5. One audited command surface shared by GUI, CLI, console, MCP agents, and future extensions.
6. Local-first operation and professional interoperability with established aerospace formats and tools.

## Domain scope

The current implementation begins with sounding-rocket-class vehicles because that is where its validated physics and fixtures exist today. That is a technical starting domain, not an audience definition. The roadmap should deepen toward serious flight dynamics and GNC workflows without pretending unsupported fidelity.

IREC 2026 and DTEG data remain useful as one optional, versioned requirements profile and regression fixture. They must be labeled as such. No default UI, demo, or project description should imply that Ascent exists for that competition.

## Decision test

For every roadmap, architecture, and UI decision, ask:

> Would a GNC or flight-dynamics engineer who works with Basilisk, Nyx, RocketPy, or GMAT respect this tool if they opened it?

Judge the answer by capability and craft, not by customer appeal.
