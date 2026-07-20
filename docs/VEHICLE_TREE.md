# Vehicle Tree Schema (v0.3 Step 1)

The vehicle model tree (`crates/ascent-domain/src/vehicle.rs`) is the single source of truth: mass properties, Barrowman aero, and the render mesh all derive from it (Step 3), and the command spine (Step 2) is the only thing that mutates it.

## Conventions

- **Datum:** nose tip. `x` grows aft. SI meters internally; masses in grams (matching the existing `dry_mass_g` convention at the sim boundary).
- **Structural parts** — `NoseCone`, `BodyTube`, `Transition` — stack in root-vector order and define the airframe. `stack_length_m` is their length sum.
- **Attachments** — `FinSet`, `MotorMount`, `Parachute`, `MassComponent` — are children of exactly one structural part, never root-level, never nested further. `position_m` fields are measured from the parent's fore end.
- **`PartId`** is a stable `u32`, unique per vehicle, never reused — commands and mesh part ranges address parts by it.
- **Every part carries explicit design mass.** `Part.as_built_mass_g` is an optional measured-hardware override; mass, CG, MOI, and stage rollups prefer it when present, while `kind.mass_g` remains the design value for reconciliation.

## Part parameters

| Part | Parameters |
|---|---|
| `NoseCone` | `shape` (`tangent_ogive` \| `conical`), `length_m`, `base_radius_m`, `mass_g` |
| `BodyTube` | `length_m`, `outer_radius_m`, `wall_mm`, `mass_g` |
| `Transition` | `length_m`, `fore_radius_m`, `aft_radius_m`, `mass_g` |
| `FinSet` | `count`, `root_chord_m`, `tip_chord_m`, `span_m`, `sweep_m`, `thickness_mm`, `mass_g` (total for the set); trailing edge flush with parent aft end |
| `MotorMount` | `motor_designation`, `length_m`, `position_m`, `mass_g` |
| `Parachute` | `diameter_cm`, `cd`, `position_m`, `mass_g` |
| `MassComponent` | `name`, `position_m`, `mass_g` |

Every `Part` may also carry `as_built_mass_g` (optional, finite, non-negative). It is deliberately a part-level field, so the same journal command applies to every part kind without changing its geometry schema.

## Mass-property worksheet model

`mass_properties()` returns total mass, CG from nose, and pitch/yaw MOI about the CG. Per-part approximations (mirrored by fixture tests):

| Part | CG station | Own MOI |
|---|---|---|
| NoseCone | fore + 2L/3 (thin conical shell — same convention as ascent-aero) | thin rod, `m·L²/12` |
| BodyTube / Transition | fore + L/2 | thin rod, `m·L²/12` |
| FinSet | parent aft − root_chord/2 | point mass |
| MotorMount | fore + position + L/2 | thin rod, `m·L²/12` |
| Parachute / MassComponent | fore + position | point mass |

Parallel-axis transfer everywhere: `I = Σ I_own + m·(x − x_cg)²`. These are deliberate slender-body approximations, honest for model-rocket aspect ratios; refinements (ogive-shell CG, fin-plate inertia) replace individual rows without changing the contract.

## Invariants (`validate()`)

1. Root parts are structural; attachments only as children of structural parts; attachments have no children.
2. `PartId`s unique across the whole tree.
3. Masses non-negative.

## Reference vehicle

`reference_vehicle()` is the Estes Alpha III dimensioned to the currently validated configuration: 25 mm caliber, 12-caliber stack (3-caliber tangent-ogive nose + 9-caliber tube — the same proportions the mesh and planar model used provisionally), fins root 2 cal / tip 1 cal / span 1.5 cal, total dry mass exactly 34.0 g. The per-part mass split (nose 8, tube 15, fins 6, chute 3, mount 2 g) is a documented estimate summing to the validated total; refine with teardown data, keeping the total pinned.

## Serialization

Serde with `type`-tagged part kinds, snake_case. TOML roundtrip is byte-identical (tested); the tree embeds in the `.ascent` project file from Step 2 onward. Unknown-field tolerance follows the project file's forward-compat policy.
