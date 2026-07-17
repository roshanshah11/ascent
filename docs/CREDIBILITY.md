# Credibility scorecard

This document defines Ascent's intended **NASA-STD-7009-inspired** credibility
scorecard. When implemented, it will be an evidence-and-limits summary, not an
assertion of NASA certification or compliance. The inspiration and the
rationale for labeling extrapolation are described in
[Professional software research](PROFESSIONAL_SOFTWARE_RESEARCH.md) §3, which
links the NASA standard and the ASME V&V extrapolation guidance.

The scorecard is designed to help a reader see what has been checked, what
inputs support a result, and where the present model no longer has supporting
evidence. It does not certify flightworthiness, ensure accuracy, or replace
range-safety or competition approval.

## Factors and 0–4 rubrics

Every result is scored on exactly these five factors, in this fixed order:

1. Verification
2. Validation
3. Input Pedigree
4. Uncertainty
5. Regime Applicability

Each factor has an integer score from 0 through 4. A higher score means more
specific, reviewable evidence for that factor; it is not a probability of being
correct.

| Score | Verification | Validation | Input Pedigree | Uncertainty | Regime Applicability |
| --- | --- | --- | --- | --- | --- |
| 0 | No check is identified. | No comparison is identified. | Inputs have no recorded source. | No uncertainty or limitation is stated. | The operating regime is unknown or outside the model's stated domain. |
| 1 | An informal or manual check is described, but is not reproducible. | A qualitative or non-comparable reference is cited. | Some inputs are named, but their origin or version is incomplete. | A limitation is named without a source or usable bound. | The result is near or beyond a stated boundary with no applicability rationale. |
| 2 | A repeatable check exists, but it covers only part of the quantity or path. | One relevant comparison exists with limited coverage or traceability. | Major inputs are sourced, though some transformations or assumptions are undocumented. | Important assumptions or sensitivity concerns are recorded, but coverage is incomplete. | The regime is partly represented by evidence, with material unmatched conditions. |
| 3 | Checked-in tests or fixtures cover the quantity and can detect a meaningful regression. | A traceable external/reference comparison covers the quantity under a stated configuration. | Inputs, assumptions, and transformations are traceable to versioned sources. | Known residuals, tolerances, and material model limits are stated. | The requested conditions are within a stated, evidence-backed regime, with documented caveats. |
| 4 | Independent or broad checks cover expected and boundary behavior with maintained regression evidence. | Multiple independent comparisons or a maintained validation library cover the decision-relevant quantity. | Inputs are versioned, reviewable, and controlled across the full calculation chain. | Quantified uncertainty is maintained for the decision-relevant conditions and its limits are clear. | The requested conditions are directly covered across the decision-relevant regime, including defined boundaries. |

## Basis and evidence rule

Every factor score must carry a human-readable `basis` string. A `basis` is
required even for a score of 0, where it explains the missing evidence. Each
basis must cite at least one checked-in file or fixture so a reader can inspect
the supporting evidence rather than accept an untraceable label.

For the present reference case, appropriate basis citations include:

- [`docs/EVIDENCE.md`](EVIDENCE.md), which records the reference configuration,
  residuals, tolerances, assumptions, and v0.1 limits;
- `data/reference/openrocket-alpha3-c6.csv`, the frozen OpenRocket table;
- `crates/ascent-sim/tests/fixtures/openrocket_alpha3_c6.json`, the frozen
  headline-value fixture; and
- `crates/ascent-sim/tests/openrocket_reference.rs`, the regression tests for
  that fixture.

For example, a Validation basis may say: "Compared with the frozen OpenRocket
reference configuration; see `docs/EVIDENCE.md` and
`crates/ascent-sim/tests/fixtures/openrocket_alpha3_c6.json`." The string is
readable prose, but the checked-in citation is mandatory.

## Quantity regime flags

Ascent must label every reported quantity as either `Validated` or
`Extrapolated`. The flag is per quantity, not a blanket property of an entire
simulation.

- `Validated` means the quantity is within a named evidence-backed reference
  regime. It does not mean the quantity is guaranteed accurate outside the
  stated evidence or safe for flight.
- `Extrapolated` means the quantity is outside that regime or has a material
  evidence gap. It must include a human-readable, named reason identifying the
  boundary or missing evidence.

At present, the evidence boundary for Ascent's vertical, subsonic, no-wind
reference configuration is the named comparison in `docs/EVIDENCE.md` with
OpenRocket 24.12's bundled **"A simple model rocket"** example, an Estes C6-5,
and the documented default launch conditions. The OpenRocket comparator is a
3-D flight with 2 m/s wind; it is not itself a vertical or no-wind
configuration. Quantities within Ascent's stated reference regime may be
labeled `Validated` only when their basis cites the corresponding frozen
OpenRocket data, fixture, or regression test named above.

An apogee prediction that is supersonic, or otherwise outside that vertical,
subsonic reference configuration, must be labeled `Extrapolated`. Its reason
must name why, for example: "Extrapolated: predicted max Mach is supersonic;
the evidence covers Ascent's vertical, subsonic, no-wind model only through a
comparison against a 3-D OpenRocket flight with 2 m/s wind," or
"Extrapolated: wind-driven trajectory is outside Ascent's vertical, no-wind
reference-model regime." A flag must never conceal the reason that supporting
evidence does not apply.

## Implementation contract

The later Rust/UI implementation must preserve the following contract:

- Emit the five factors in the deterministic order listed in this document.
- Constrain every factor score to the inclusive integer range 0–4.
- Require a non-empty, human-readable `basis` for every factor score, including
  its checked-in evidence citation.
- Attach a `Validated` or `Extrapolated` flag to every reported quantity; an
  `Extrapolated` quantity also requires its named reason.
- Present both factor scores and per-quantity flags in the UI so an evidence
  drawer reader can see the supporting basis and limits.

The UI is a communication surface for evidence and limits. It must not present
the scorecard as certification, a guarantee of accuracy, or approval for a
launch, range, or competition.
