# ASCENT — Idea Spec

*Reframed 2026-07-19. See `PRODUCT_DIRECTION.md` for the authoritative project context.*

## The idea

Ascent is a professional aerospace engineering environment that unifies vehicle modeling, flight dynamics, uncertainty analysis, requirements verification, results, and evidence in a native, local-first workbench.

Its distinctive architecture makes an AI agent a first-class client of the same audited command layer used by the engineer. The agent reads state, proposes typed changes, exposes their exact effects and stale-study impact, and applies them only after approval. The simulator and evidence system verify outcomes; the model never receives a hidden mutation path.

## The empty seat

Basilisk, Nyx, GMAT, and RocketPy establish the professional bar for simulation architecture, validation, and flight dynamics. They are powerful specialist tools and libraries. Ascent's opportunity is not to dilute that rigor for a beginner market. It is to place comparable discipline inside a real workbench with a model tree, managed studies, uncertainty views, evidence, undo, visualization, and programmable operation.

OpenRocket and historical competition datasets remain useful for file compatibility, regression fixtures, and independent comparisons. Their existence does not define Ascent's audience or ambition.

## Measuring user

A GNC or flight-dynamics engineer at a new-space or defense-tech startup should be able to open Ascent and recognize:

- honest model boundaries and units;
- reproducible seeded studies;
- validation against named baselines and real data;
- uncertainty and credibility shown instead of hidden;
- traceable requirements and evidence;
- modular, scriptable, automation-safe architecture;
- an efficient native workbench rather than a web dashboard or chat wrapper.

## Core principles

1. **Rigor before spectacle.** Every physics claim has a validity regime and evidence.
2. **Determinism is infrastructure.** Same inputs and seed produce the same bytes.
3. **One command spine.** GUI, CLI, console, MCP, and extensions share the mutation path.
4. **AI proposes; engineering verifies.** Approval, diffs, solver results, and evidence stay explicit.
5. **Uncertainty is a first-class result.** Dispersions and model disagreement are inspectable work objects.
6. **Interoperability earns trust.** Compare with established tools and reconcile predictions against flight data.
7. **Professional UX supports deep work.** Stable spatial anatomy, keyboard operation, visible computation state, and dense legible data.

## Scope

The implementation starts with sounding-rocket-class flight because the repository already contains validated motors, aerodynamics, atmosphere, recovery, dispersion, and six-degree-of-freedom foundations. This is a technical foothold for deeper flight-dynamics and GNC capability, not a student or competition-market wedge.

Ascent is currently a serious craft project rather than a product being sold. Do not introduce customers, pricing, sales, market sizing, or education-market plans into its direction.
