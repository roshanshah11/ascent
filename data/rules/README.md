# IREC 2026 optional requirements profile

`irec-2026.json` is a source-traceable, versioned example of Ascent's generic requirements-profile system. It is retained as an optional engineering fixture because its official sources provide concrete vehicle, trajectory, stability, recovery, and reporting constraints. It does not define Ascent's audience or default product identity.

When this optional profile is active, its output is an engineering aid, not official approval. IREC officials, the Range Safety Officer, and the current official documents remain authoritative.

## Primary sources

Only these two official documents supply rule values:

1. **International Rocket Engineering Competition Rules & Requirements Document**, `2026 ver. 1.0`, effective October 5, 2025, [official PDF](https://www.esrarocket.org/s/IREC-Rules-and-Requirements-Document-2026-v10-f8wf.pdf).
2. **International Rocket Engineering Competition Design, Test, & Evaluation Guide**, `2026 V1.1`, effective February 23, 2026, [official PDF](https://www.esrarocket.org/s/2026-IREC-DTEG-V11.pdf).

The files were retrieved on July 17, 2026. Their SHA-256 digests are stored in the JSON `sources` array so a future build can detect upstream replacement at the same URL.

## Rule schema

Each item in `rules` has the required fields plus enough typing to prevent unlike values from being treated as interchangeable:

| Field | Meaning |
|---|---|
| `id` | Stable, unique identifier. |
| `description` | Human-readable statement that preserves important conditions. |
| `quantity` | Machine-oriented name of the measured or configured quantity. Altitudes include `agl` or `msl` where relevant. |
| `comparator` | Source relationship such as `gte`, `gt`, `lte`, `lt`, `between`, `target`, `within_plus_minus`, or `typical_plus_minus`. |
| `value` | Typed numeric value. See value forms below. |
| `units` | Units and datum exactly needed to interpret `value`. |
| `applicability` | Apogee categories, COTS/SRAD origins, propulsion types, stage configurations, and conditional qualifications. Target rules also contain authoritative `allowed_configurations` tuples. |
| `authority` | Distinguishes requirements from scoring rules, assumptions, exemptions, published equipment, and operational defaults. |
| `evaluation_scope` | Where Ascent should use the item: design, simulation, flight control, inspection, or postflight scoring. |
| `citation` | Exact official document, version, subsection, page, URL, and supporting quote. |

Optional `source_equivalents` reproduce alternate units printed by the source. They are not recalculated conversions and should not be assumed more precise than the source.

### Value forms

- Scalar: `{ "kind": "scalar", "number": 25 }`
- Range: `{ "kind": "range", "lower": 20, "upper": 40, "boundary_semantics": "source_unspecified" }`
- Relative value: `{ "kind": "relative", "number": 70, "reference_quantity": "simulated_desired_firing_altitude" }`
- Relative tolerance: `{ "kind": "relative_tolerance", "number": 30, "reference_quantity": "selected_target_apogee" }`
- Nominal with tolerance: `{ "kind": "nominal_with_tolerance", "nominal": 84, "tolerance": 2 }`

Consumers should reject unknown comparator/value-kind combinations rather than guessing.

## Applicability rules

The valid scored category combinations come from Rules §2.0 and are encoded by the three target-apogee records. Their structured `allowed_configurations` arrays are authoritative:

- 10K single-stage: COTS solid/hybrid or SRAD solid/hybrid/liquid.
- 30K single-stage: COTS solid/hybrid or SRAD solid/hybrid/liquid.
- 30K two-stage: any propulsion type.
- 45K two-stage: any propulsion type.

The summary arrays alongside `allowed_configurations` are indexes for filtering, not Cartesian products. Applicability arrays on all other rules describe broad coverage over the valid configurations established by the target rules; they do not create new category combinations. In particular, there is no COTS-liquid single-stage category and no 45K single-stage category.

## Important interpretation boundaries

### Stability percentages are not fixed calibers

DTEG §§10.2.1.1–10.3.1.2 publish the numeric stability limits only as percentages of total rocket length: 7.5% minimum subsonic, 10% minimum supersonic, 18% maximum at launch, and 25% maximum through flight. Section 10.2.3 requires the flight curve to be **reported in calibers** under a no-wind simulation and initial stability to be reported in percent, but it gives no numeric caliber limits.

The pack therefore does not invent fixed caliber equivalents. A conversion would depend on the specific rocket's length-to-diameter ratio.

### Fin span terminology is unresolved in the sources

DTEG §10.2.4 says “fin span” must not be less than 0.8 calibers. The DTEG revision summary says “fin semi-spans,” while Rules §2.6.2.9.1 asks teams to report “Fin semi-span.” The JSON preserves the normative §10.2.4 wording and uses `fin_span_as_defined_by_dteg_relative_to_largest_tube_diameter` rather than silently selecting a geometry convention.

### Launch angle is an operational default

DTEG §10.1.1 says launch officials set the launch elevation. The 84° ±2° single-stage and 87° ±2° multi-stage values are typical conditions, not hard pass/fail constraints. The often-used 6° from vertical is derived from the 84° typical elevation and is not encoded as an independent official rule.

The `0° launch angle` in DTEG §5.13.1.2 applies only to the simulation used to set an air-start altitude lockout. The document does not explicitly reconcile that angle convention with §10.1.1's elevation-above-horizontal convention.

### Analysis conditions are scoped, not global

- The no-wind condition in DTEG §10.2.3 applies to the required stability-margin simulation.
- The no-wind condition in §5.13.1.2 applies to the air-start lockout simulation.
- The 890 m MSL site elevation in §§6.3.2 and 6.4.2 applies to descent-rate calculations.

None of these is treated as a universal assumption for every apogee or trajectory analysis.

### Recovery boundaries preserve source wording

DTEG §6.1.1 requires dual-event recovery for an anticipated apogee **above** 457 m AGL, while §6.1.2 exempts independently recovered payloads released **below** 457 m AGL. The exact 457 m boundary is not resolved by those two statements. Section 6.4.1 separately requires main deployment no higher than 457 m AGL.

Section 6.3.3 says the drogue descent rate shall be “between 20-40 m/s” but does not state whether the endpoints are inclusive. Its range is tagged `boundary_semantics: source_unspecified`.

### Conditional rules are not unconditional thresholds

DTEG §7.4.1 allows active controls to leave their neutral state after any one of three alternatives: boost ends, the vehicle passes max-Q, or the category-specific altitude is reached. The 2,000 m and 6,000 m entries are therefore explicitly labeled OR alternatives, not universal minimum activation altitudes.

## Scope and exclusions

The JSON includes numeric items that directly affect the competition flight configuration, simulated flight, stability and structure checks, recovery behavior, or apogee/recovery scoring. It includes typed non-pass/fail values when they are necessary simulation inputs, such as published rail lengths and typical launch elevations.

It intentionally excludes numeric requirements that do not govern flight performance, including submission deadlines and penalties, postflight desk-review time limits, arming distances, pressure-vessel proof factors, launch-controller range, wiring/environmental qualification values, and coupler/fastener construction details. It also excludes federal amateur-rocket classification ceilings quoted in the Rules glossary and the non-competing `50k ft+` demonstration descriptor. Those belong in operations, structures, or regulatory rule packs rather than this flight-review pack.

Qualitative requirements are not converted into fake numbers. For example, initial recovery deployment must occur “at or near apogee,” but neither source defines a numeric time or altitude tolerance.

## Provenance and maintenance

- Every numeric rule has a citation with an exact subsection, not just a page.
- Quotes retain the source's numeric wording and comparator.
- Alternate units are copied from the official document; no new converted constants are asserted.
- Scoring targets and operational defaults must not be rendered as safety approval.
- When IREC publishes a revision, add a new versioned pack or explicitly review every rule before changing this one.

Recommended validation before use:

1. Parse the JSON.
2. Require unique rule IDs.
3. Require all mandatory fields and nonempty citation sections.
4. Restrict citation document/version pairs to the `sources` array.
5. Validate comparator/value-kind combinations and ordered range bounds.
6. Keep AGL and MSL quantities distinct.
7. Confirm no 45K single-stage category is introduced.
8. Confirm percentage-of-length stability values are never labeled as calibers.
