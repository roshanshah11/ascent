# Evidence — Ascent vs OpenRocket 24.12 (Day 3 golden fixture)

Every physics claim must be verifiable against a reference. This page is that
verification for the flight kernel.

## Reference

OpenRocket 24.12, bundled **"A simple model rocket"** example, Estes **C6-5**,
default launch conditions (Cape Canaveral site: latitude-adjusted gravity
9.792 m/s², 15 °C, 1013.25 mbar, 2 m/s wind, 1 m launch rod). Manually
exported simulation table: `data/reference/openrocket-alpha3-c6.csv`
(650 rows, 58 variables, event markers). Frozen headline values:
`crates/ascent-sim/tests/fixtures/openrocket_alpha3_c6.json`.
Regression tests: `crates/ascent-sim/tests/openrocket_reference.rs`.

Note: this example rocket is *not* the Estes Alpha III kit — it is heavier
(71.26 g at liftoff vs ~58 g). Same 25 mm diameter, same motor class. All
comparisons below use Ascent configured to match this vehicle.

## Matched configuration

| Parameter | Value | Where it came from |
|---|---|---|
| Liftoff mass | 71.259 g | OpenRocket mass column at t=0; Ascent dry mass set to 71.259 g − our motor's 24.1 g so totals match exactly |
| Burnout mass | 60.459 g | identical in both tools (10.8 g propellant, same NAR cert data) |
| Reference area | 4.909 cm² | identical (25 mm diameter) |
| Drag Cd | 0.63 constant | OpenRocket's own computed coast-phase Cd from the export |
| Chute | 30 cm, Cd 0.8 | derived from the export: post-deploy drag coefficient 115.2 × 4.909 cm² = cdA 0.0566 m² |
| Gravity | 9.792 m/s² | OpenRocket's latitude-adjusted value |
| Rail | 1.0 m | OpenRocket default rod |

## What agrees

| Quantity | OpenRocket | Ascent | Δ | Test tolerance |
|---|---|---|---|---|
| Burnout time | 1.860 s | 1.860 s | 0.0% | 1% |
| Burnout altitude | 102.6 m | 102.9 m | +0.3% | 5% |
| Burnout velocity | 94.55 m/s | 94.96 m/s | +0.4% | 2% |
| Max velocity | 95.3 m/s | 95.37 m/s | +0.1% | 2% |
| **Apogee** | **316.8 m** | **321.2 m** | **+1.4%** | 5% |
| Rail-exit velocity | 17.80 m/s | 17.37 m/s | −2.4% | 10% |
| Descent rate (steady) | 4.17 m/s | 4.12 m/s | −1.2% | 10% |
| Flight time | 83.6 s | 85.4 s | +2.1% | 10% |

## What differs, and why

- **Apogee time: 7.278 s (OR) vs 7.824 s (Ascent), +7.5%.** Not a physics
  disagreement. OpenRocket fires the ejection charge at burnout + 5 s
  (6.861 s) — *before* apogee — and the deployed chute kills the remaining
  climb almost instantly, so its apogee registers at 7.278 s having gained
  only 1.4 m after deploy. Ascent deploys at apogee and coasts ballistically
  to the true vertex. Altitude is nearly unaffected (the +1.4% above);
  only the timestamp shifts. Test tolerance 10%, for exactly this reason.
- **Apogee +1.4% high.** Expected direction: OpenRocket flies 3D in a 2 m/s
  wind, so angle-of-attack drag and the slightly tilted trajectory cost it
  altitude a vertical no-wind point-mass doesn't pay. A vertical sim should
  slightly beat, never badly trail, the 3D result.
- **Rail-exit velocity −2.4%.** The tools measure rod departure differently:
  OpenRocket tracks travel along the rod from the initial CG position; Ascent
  crosses an altitude equal to rod length. Same physics, different trigger.
- **Cd during burn.** OpenRocket's Cd is Mach/Reynolds-dependent (0.87 static
  falling to ~0.63 in coast); Ascent uses one constant. Matching at the coast
  value makes the coast (which dominates apogee) right and leaves a small
  error during the 1.86 s burn — visible as the +0.4% burnout velocity.
- **Descent.** The chute cdA was *derived from the same export*, so the
  descent-rate row checks Ascent's descent integration, not an independent
  chute model. Flight time is the more independent descent check (apogee
  height + descent rate together determine it). OpenRocket's summary "ground
  hit velocity" (4.57 m/s) includes lateral wind drift; the clean vertical
  comparison is its mid-descent rate (4.17 m/s at 180 m).

## Known model assumptions (Ascent v0.1)

1D vertical flight, point mass, no wind, constant Cd, chute at apogee,
troposphere-only standard atmosphere, fixed-step RK4 with event
interpolation. Each is either matched or accounted for above.

## Verdict

With mass, drag, and gravity matched, Ascent's vertical kernel reproduces
OpenRocket's ascent to within 1.4% on apogee and 0.1% on max velocity, with
every residual difference traced to a named modeling choice. The Day 3 exit
gate — "evidence page states what agrees, what differs, why" — is met.
