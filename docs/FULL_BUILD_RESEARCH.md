# ASCENT — Full-Product Research and Build Blueprint

*Generated: 2026-07-16 | Sources: 24 cited | Confidence: high on the initial workflow and architecture; medium on future solver bridges*

This document separates the July 23 proof artifact from the complete product. It is based primarily on 2026 IREC requirements, OpenRocket and RocketPy documentation/source, ThrustCurve's API contract, and official documentation for the proposed desktop and fidelity-stack technologies.

## Executive decision

Ascent should not initially be framed as a generic “AI-native rocket simulator.” Its strongest application is:

> **Design a competition rocket to hit a target apogee, prove that it satisfies flight-review constraints, and produce a reproducible analysis package.**

That is a complete job for a concrete user. The 2026 IREC materials require teams to provide predicted apogee, rail-departure velocity, stability through flight, a simulation file, and an altitude/velocity/acceleration graph under prescribed conditions. Flight scoring also rewards closeness to a 10,000, 30,000, or 45,000 ft target, while a separate modeling-and-simulation award compares predicted with actual apogee ([IREC 2026 Rules](https://www.esrarocket.org/s/IREC-Rules-and-Requirements-Document-2026-v10-f8wf.pdf), [IREC 2026 DTEG](https://www.esrarocket.org/s/2026-IREC-DTEG-V11.pdf), [IREC Progress Report](https://www.esrarocket.org/s/2026-IREC-Editable-Progress-Report-1-11-24-25.pdf)).

The product loop is therefore:

1. Build or import the vehicle.
2. Select a certified motor and payload.
3. Simulate with declared assumptions.
4. Check apogee, rail speed, stability, recovery, and rule compliance.
5. Change the design or ask Ascent to find a feasible configuration.
6. Compare predictions with another engine or real flight data.
7. Export the evidence.

The simulator is the correctness core inside this application. It is not the product by itself.

## What the research changes in the current specs

| Current assumption | Research finding | Product decision |
|---|---|---|
| “AI-native” is the lead | The highest-value user job is a verified competition flight review. AI is not required to complete it. | Lead with **rocket engineering and target-flight design**. Add the copilot only after deterministic design actions and verification exist. |
| “6-DOF-lite (3-DOF + attitude)” | This is technically ambiguous. OpenRocket and RocketPy both implement genuine 6-DOF; a point-mass model should be named honestly ([OpenRocket technical documentation](https://openrocket.sourceforge.net/techdoc.pdf), [RocketPy equations](https://github.com/RocketPy-Team/RocketPy/blob/master/docs/technical/equations_of_motion.rst)). | Ship an explicitly scoped **2D point-mass competition simulation** first. Build a 3D point-mass model and then full rigid-body 6-DOF later. |
| Copilot on day seven | A cloud LLM conflicts with the $0 requirement and distracts from the anti-slop evidence. | Day seven becomes the **IREC Flight Review + target-apogee solver**. A real copilot is post-demo. |
| Live ThrustCurve integration immediately | The API is usable, but network dependence and per-file license/source metadata add failure modes. The API can return parsed samples and labels each file as public-domain, free, or other ([API docs](https://www.thrustcurve.org/info/api.html), [OpenAPI schema](https://www.thrustcurve.org/api/v1/swagger.json)). | Bundle three provenance-preserving demo motors. Add API search, caching, and license filtering after the core works. |
| OpenVSP is automatically a higher-fidelity aero tier | OpenVSP describes itself as a parametric **aircraft geometry** tool. Its API is excellent for geometry automation, but using it does not automatically improve rocket drag or transonic predictions ([OpenVSP README](https://github.com/OpenVSP/OpenVSP), [OpenVSP API](https://openvsp.org/api_docs/latest/)). | Treat OpenVSP first as geometry/CAD interoperability. Require a validated rocket-specific experiment before calling VSPAERO a fidelity upgrade. |
| GMAT is a natural near-term bridge | GMAT is for orbital and deep-space mission design, navigation, optimization, and targeting, not the immediate unguided competition-flight job ([NASA GMAT](https://github.com/nasa/GMAT)). | Remove GMAT from the active roadmap. Reconsider only if Ascent moves into orbital mission design. |
| OpenFOAM can become a quick Mac button | The OpenFOAM Foundation's supported macOS route runs Ubuntu through Canonical Multipass, and a trustworthy CFD workflow also needs geometry preparation, meshing, convergence checks, and validation ([OpenFOAM for macOS](https://openfoam.org/download/macos/)). | Keep CFD as a later external case-generation and evidence workflow, not a v0.x feature. |
| OpenRocket was last released in March 2026 | As of this research, GitHub lists 24.12, published July 27, 2025, as the latest stable release; 26.xx changes are present on the unstable branch ([OpenRocket releases](https://github.com/openrocket/openrocket/releases), [file-format history](https://github.com/openrocket/openrocket/blob/unstable/fileformat.txt)). | Cite stable and development behavior separately. Do not build strategy on the claim that OpenRocket is abandoned. |

## The application to build

### Core user

The initial user is a collegiate competition team designing a single-stage COTS-motor rocket for an IREC-style target-altitude category. This is narrower and more actionable than “everyone below professional aerospace.” High-power hobbyists remain a second ring, but the competition workflow determines the first product.

### Completed job

“Given our geometry, measured mass properties, payload, motor, launch site, and recovery system, show whether the vehicle will safely leave the rail, remain within stability limits, reach the target altitude, and recover; then give us the evidence package and a reproducible record of the run.”

### Required outputs

The 2026 IREC evidence establishes a useful minimum output set:

- liftoff mass and configuration;
- launch-rail departure velocity, with 25 m/s as the stated minimum in the progress-report form;
- maximum acceleration and velocity;
- predicted apogee and error from the selected target;
- CG, CP, and stability through first recovery deployment;
- altitude, velocity, and acceleration versus time;
- recovery event timing and landing speed;
- simulation conditions and assumptions;
- a shareable simulation file and graph/report artifact.

The DTEG specifies minimum stability of 7.5% of rocket length for subsonic flight and 10% for supersonic flight. It also limits stability to 18% at launch and 25% through flight, and requires fin span of at least 0.8 calibers. These should be first-class constraints, not buried warnings ([IREC DTEG, section 10](https://www.esrarocket.org/s/2026-IREC-DTEG-V11.pdf)).

## The 90-second July 23 demo

The demo should tell one continuous story:

1. Open **IREC 10K — single-stage COTS**. The rocket is visible in the center; CP and CG markers sit on its body. The review strip shows target apogee, rail speed, and stability.
2. Increase payload mass or select a different motor. The geometry, CG, stability band, and predicted apogee update. One constraint turns red.
3. Press **Find feasible configuration**. A deterministic search tries allowed motors and ballast values. It returns a before/after design diff that hits the target while keeping rail speed and stability valid.
4. Press **Launch**. The rocket flies; altitude, velocity, and acceleration traces draw live. Burnout, rail exit, apogee, chute deployment, and landing appear on an event timeline.
5. Open **Evidence**. Show the exact input hash, model version, assumptions, convergence status, and a comparison against one frozen reference run.

That demo displays a physical object, a serious computation, application state, correctness evidence, and an actual decision. It does not require an LLM.

## Feature map

| Capability | July 23 vertical slice | 30-day product | Multi-month product |
|---|---|---|---|
| Vehicle design | One stage; nose, body, fins, payload, motor, parachute; direct parametric editing | Component tree, materials, mass/CG overrides, presets, undo/redo | Multi-stage, pods, boosters, free-form fins, measured assemblies |
| Visualization | High-quality 2D/3D model, CP/CG markers, animated launch | Section view, internal components, measurement tools | CAD exchange and manufacturing drawings |
| Motors | Three bundled, sourced thrust curves | ThrustCurve search/cache; RASP and RockSim imports | Motor uncertainty, clustering, air starts, SRAD motors |
| Simulation | Deterministic no-wind point-mass flight with rail, burn, coast, recovery | Wind, atmosphere profiles, recovery configuration, batch runs | Full rigid-body 6-DOF, staging, roll, control surfaces |
| Stability/aero | Barrowman static CP and time-varying CG; declared drag model | Mach-dependent coefficient curves and warnings | Aero lookup tables, calibrated models, validated external solvers |
| Competition review | IREC 10K rule profile and evidence summary | Versioned rule packs, PDF export, `.ork` subset export | Team sign-off, requirements traceability, range-safety packages |
| Optimization | Motor/ballast enumeration against constraints | Continuous parameter search and sensitivity | Robust design under uncertainty, Pareto trade studies |
| Validation | Unit, invariant, convergence, and one cross-engine golden case | RocketPy adapter and flight-data overlay | Multi-engine envelopes, model calibration, uncertainty budgets |
| Copilot | Not in the critical path | Typed, verification-gated design suggestions | Goal decomposition, trade explanations, team knowledge |

## Recommended technical architecture

Keep the physics independent of Tauri, JavaScript, storage, and any AI provider.

```text
ascent/
├── crates/
│   ├── ascent-domain/       # versioned vehicle, motor, environment, units, constraints
│   ├── ascent-aero/         # CP, CG, stability, drag and atmosphere models
│   ├── ascent-sim/          # state, RK integrator, event detection, recovery
│   ├── ascent-optimize/     # deterministic searches over typed design actions
│   ├── ascent-validation/   # fixtures, comparisons, tolerances, provenance
│   └── ascent-interop/      # RASP/RSE, CSV, .ork subset, RocketPy adapters
├── src-tauri/               # thin commands, async jobs, file access, packaging
├── src/                     # TypeScript application and rendering
├── fixtures/                # versioned motors, vehicles, reference outputs
└── schemas/                 # project and run-record schemas
```

For the nine-day build, these may begin as modules in one Rust crate. Preserve the dependency direction from the first commit: `domain <- aero <- sim <- optimize`; interop and the app call inward, never the reverse.

Tauri is a reasonable fit because it combines a Rust core with an OS WebView and message-passing APIs rather than bundling a browser runtime ([Tauri architecture](https://v2.tauri.app/concept/architecture/)). The frontend should issue coarse commands such as `simulate(design, conditions)` and receive a completed summary plus a bounded time series. Do not send an IPC message for every integration step.

### Frontend recommendation

- React + TypeScript + Vite for the application shell.
- Three.js for a parametric viewport if the first rocket can be rendered from revolved profiles and duplicated fin meshes without importing CAD.
- SVG or Canvas for CP/CG overlays and the competition-review strip.
- uPlot or a similarly lightweight canvas plotter for synchronized telemetry.
- A single immutable application store with explicit design revision and run status.

The UI must remain usable if the 3D viewport is replaced by a 2D side view. The product value is the verified interaction loop, not a fragile rendering stack.

## Domain model and file format

### Internal conventions

- Store all physics in SI units.
- Put unit suffixes on boundary fields (`length_m`, `mass_kg`, `thrust_n`) in v0.1. Introduce stronger unit types after the core stabilizes.
- Give every component and run a stable UUID.
- Separate user inputs from derived values. Never serialize computed CP, apogee, or plots as design truth.
- Version every schema from day one.

### Project structure

A Git-friendly project should be a directory, not a binary monolith:

```text
Falcon-10K.ascent/
├── project.toml
├── vehicle.json
├── configurations.json
├── motors/
│   └── aerotech_m1939.rse
├── runs/
│   └── 2026-07-16T2204Z.json
└── attachments/
```

`vehicle.json` is ordered and explicit. `project.toml` holds metadata and schema version. Run records are immutable derived evidence and can be omitted from source control if a team chooses. A zipped `.ascentpkg` can be added for sharing without sacrificing the directory representation.

### Run record

Every simulation run should record:

- normalized design/configuration hash;
- simulator semantic version and Git commit;
- model identifiers and validity warnings;
- motor-curve digest and source/license;
- launch conditions and random seed;
- integrator, timestep/tolerances, and event settings;
- scalar results, event log, and optionally compressed samples;
- validation status and reference comparison.

This makes “why did the prediction change?” answerable.

### Application state

Use an explicit state machine:

```text
Clean design ──edit──> Dirty design ──simulate──> Running
     ▲                                             │
     └──────── result matches design hash ────────┤
Dirty design <── stale result / later edit ───────┘
```

A completed result may update the screen only if its input hash still matches the current design. This prevents a slow prior simulation from overwriting a newer edit. OpenRocket itself visually distinguishes current and stale simulation results; Ascent should make this behavior part of its domain state, not only a UI color ([OpenRocket technical documentation](https://openrocket.sourceforge.net/techdoc.pdf)).

## Physics-core specification

### v0.1 scope: an honest point-mass model

For the first release, model a single-stage rocket in a vertical plane. Suggested state:

```text
y = [downrange, altitude, vx, vz, remaining_propellant_mass]
```

Forces:

- thrust, interpolated from a measured motor curve;
- aerodynamic drag opposite air-relative velocity;
- gravity;
- rail constraint until rail exit;
- a recovery drag-area change after deployment.

This model predicts rail departure, burnout, coast, apogee, and recovery without pretending to predict six-axis attitude motion. CP and CG are evaluated as stability constraints alongside the flight, not used to imply a dynamic weathercocking model.

### Thrust and mass depletion

1. Validate motor samples: finite values, strictly increasing time, nonnegative thrust.
2. Insert `(0, 0)` and a zero-thrust burnout endpoint when the source omits them, while recording the normalization.
3. Use linear interpolation for thrust.
4. Compute total impulse with trapezoidal integration.
5. In the absence of a measured mass-flow curve, deplete propellant in proportion to cumulative impulse so that propellant mass reaches zero at burnout. Label this assumption.

ThrustCurve's download API can return either original RASP/RockSim files or normalized time/thrust samples. It also exposes manufacturer, dimensions, impulse, burn time, propellant mass, certification source, availability, and data provenance ([ThrustCurve OpenAPI](https://www.thrustcurve.org/api/v1/swagger.json)). Cache the selected curve and its metadata locally so a saved run never depends on the future state of the API.

### Atmosphere and gravity

- v0.1: implement the relevant layers of the 1976 standard atmosphere, at minimum through the altitude of the selected competition class.
- Use air-relative velocity everywhere, even when wind is zero.
- Use constant gravity for the demo and record it. Add latitude/altitude-dependent gravity only when its effect is measurable for the target regime.

### Drag

Drag is `0.5 * rho * |v_air|^2 * Cd * reference_area`, opposite air-relative velocity.

The difficult term is `Cd`. Do not claim that a simple geometry formula is an authoritative transonic drag model. For the demo:

- use a declared constant or small Mach-indexed `Cd` table for the chosen reference rocket;
- show the active model and validity range in Evidence;
- add a sensitivity band for plausible `Cd` variation if time permits.

Later, separate drag contributors and accept imported coefficient tables. OpenRocket's unstable 1.11 file format is itself adding drag and stability lookup CSV tables, evidence that lookup-driven aero is a practical interoperability boundary ([OpenRocket file-format history](https://github.com/openrocket/openrocket/blob/unstable/fileformat.txt)).

### CP, CG, and stability

- Compute component mass and axial CG from geometry/material density or explicit overrides.
- Update total CG during motor depletion.
- Implement Barrowman normal-force and CP equations for the v0.1 component subset: nose cone, body transitions, and trapezoidal fins.
- Define static margin in both calibers and percent of total rocket length.
- Sample stability throughout ascent, not only on the pad.
- Emit separate violations for minimum stability, over-stability, and fin span.

The OpenRocket technical document remains the best implementation reference for its Barrowman extensions and clearly states the assumptions and validation limits. Use published equations and independent tests; do not copy GPL implementation code into a closed-source Rust core ([technical document](https://openrocket.sourceforge.net/techdoc.pdf), [OpenRocket GPL repository](https://github.com/openrocket/openrocket)).

### Numerical integration

Start with fixed-step RK4 because it is easy to test and adequate for the bounded demo problem. The integrator must not own flight-phase logic. Implement:

- a pure derivative function;
- an RK4 stepper;
- an event detector that brackets threshold crossings;
- root interpolation for rail exit, burnout, apogee, deployment, and ground impact;
- maximum-time and non-finite-state aborts.

Begin with `dt = 0.005–0.01 s`, then establish the actual setting through convergence tests. A run passes convergence when halving the timestep changes key metrics by less than declared tolerances. The tolerance, not “RK4,” is the evidence.

### Flight phases

```text
OnRail -> PoweredAscent -> Coast -> Recovery -> Landed
```

Each transition creates an immutable event with interpolated event time and state. Invalid orderings fail loudly. Recovery v0.1 may be a single parachute deployed at apogee with a fixed `CdA`; add delayed, staged, and inflation dynamics later.

## Validation program

Validation is the differentiating product, not a final QA task.

### Layer 1: mathematical and physical invariants

- constant-acceleration trajectory matches the analytic solution;
- zero-force velocity remains constant;
- no-drag ballistic apogee matches the energy solution;
- motor impulse matches trapezoidal integration;
- total mass never drops below dry mass;
- thrust is zero outside the curve;
- event times are ordered;
- altitude is finite and ground impact terminates;
- identical inputs and seed produce byte-stable summaries.

### Layer 2: numerical behavior

- timestep convergence at `dt`, `dt/2`, and `dt/4`;
- event-time convergence independent of sample cadence;
- interpolation tests at curve knots and boundaries;
- randomized/property tests: increasing dry mass should not increase apogee under otherwise identical conditions; increasing `CdA` should not increase apogee; adding parachute area should not increase terminal landing speed.

### Layer 3: reference-engine fixtures

Create a fixture schema that stores inputs, reference engine/version, exported outputs, tolerances, and notes. Do not test only the final apogee. Compare:

- total impulse and burnout time;
- rail departure time and velocity;
- maximum acceleration and velocity;
- apogee and time to apogee;
- recovery deployment and landing speed;
- CG/CP/stability at selected times.

OpenRocket does not document a batch-simulation CLI; its documented command-line options are application startup flags ([command-line documentation](https://github.com/openrocket/openrocket/blob/unstable/docs/source/dev_guide/command_line_arguments.rst)). For v0.1, export a reference CSV manually. Later, build a development-only Java harness against OpenRocket core or require a user-installed external adapter. Keep GPL code out of Ascent unless the licensing choice is deliberate.

### Layer 4: real-flight data

RocketPy is a strong validation partner. Its repository is MIT-licensed, its published paper documents validation, and its current acceptance tests include real EPFL Bella Lui and Notre Dame flight datasets. Those tests assert apogee error below 1.5% and time-to-apogee error below 2% for the represented cases; Bella Lui additionally checks peak velocity and acceleration ([RocketPy paper](https://doi.org/10.1061/%28ASCE%29AS.1943-5525.0001331), [Bella Lui test](https://github.com/RocketPy-Team/RocketPy/blob/master/tests/acceptance/test_bella_lui_rocket.py), [NDRT test](https://github.com/RocketPy-Team/RocketPy/blob/master/tests/acceptance/test_ndrt_2020_rocket.py)). These thresholds are properties of those configured cases, not a universal accuracy guarantee.

Start with one clean reference rocket. Add real-flight validation only after the engine reproduces its own assumptions, because motor variability, weather, measured mass, launch angle, and sensor processing can otherwise hide implementation errors.

### Validation UI

Every run receives one of four states:

- **Verified**: within declared tolerance against a named fixture and inside model validity.
- **Consistent**: invariants and convergence pass, but no external fixture applies.
- **Warning**: run completed outside a recommended model regime or with weak inputs.
- **Invalid**: violated input, numerical, or event-order constraints.

Never display “accurate” without naming the evidence.

## Competition Flight Review

Implement competition requirements as versioned data, not hard-coded UI conditionals:

```json
{
  "id": "irec-2026-single-stage",
  "target_apogee_ft": [10000, 30000, 45000],
  "site_elevation_m": 890,
  "launch_angle_from_vertical_deg": 6,
  "wind_mode": "none",
  "minimum_rail_exit_speed_mps": 25,
  "minimum_stability_percent_subsonic": 7.5,
  "minimum_stability_percent_supersonic": 10,
  "maximum_stability_percent_launch": 18,
  "maximum_stability_percent_flight": 25,
  "minimum_fin_span_calibers": 0.8
}
```

The UI should distinguish:

- a **rule failure** sourced to a rule-pack section;
- a **model warning** sourced to a physics validity regime;
- a **design recommendation** produced by Ascent;
- a **missing measurement** such as unverified finished mass or CG.

Do not claim that passing Ascent is official flight approval. The report is an engineering aid and evidence generator; competition officials and range-safety personnel remain authoritative.

## Target-apogee optimization

The first “smart” feature should be deterministic.

### v0.1 action space

- choose among a small set of compatible motors;
- add ballast within a declared interval;
- optionally change one fin parameter within manufacturing bounds.

### Search

1. Enumerate compatible motors.
2. For each motor, bracket feasible ballast.
3. Use bisection or bounded scalar search to reduce target-apogee error.
4. Reject every candidate that violates rail speed, stability, mass, or geometry constraints.
5. Rank survivors by target error, margin to constraints, and modification cost.
6. Return a typed patch and before/after simulation, never mutate silently.

This is visibly advanced, entirely local, easy to explain, and testable. Later optimization can add sensitivity and robust objectives under motor/weather uncertainty.

## Verification-gated copilot

Add an LLM only after every useful design change is represented by a typed action. The model may call:

- `inspect_design`
- `set_parameter`
- `replace_motor`
- `add_ballast`
- `run_simulation`
- `check_constraints`
- `compare_runs`
- `propose_patch`

It may not write arbitrary project files, execute shell commands, invent motor curves, or provide an unverified final number.

The execution loop is:

```text
Goal -> proposed typed patch -> schema validation -> physics run
     -> constraint evaluation -> before/after evidence -> user approval
```

Numeric claims displayed as results must come from simulation or stored measurements. LLM prose can explain tradeoffs but is not evidence. Preserve the full action/result trace in the run record. Support bring-your-own-key or a separately priced cloud feature; do not compromise the local core or the $0 demo for it.

## Interoperability

### OpenRocket `.ork`

Current `.ork` files are ZIP packages containing XML plus assets; the unstable 1.11 format adds preview images, embedded motor files, lookup tables, and other data. OpenRocket states that the reference implementation is still the effective format documentation ([file-format history](https://github.com/openrocket/openrocket/blob/unstable/fileformat.txt), [example designs](https://github.com/openrocket/openrocket/tree/unstable/core/src/main/resources/datafiles/examples)).

Build interoperability in stages:

1. Read-only import of the v0.1 component subset.
2. Preserve unknown XML/assets for round-trip safety.
3. Export one-stage designs with a single competition simulation.
4. Add a conformance corpus across format versions.
5. Display an explicit loss report whenever a feature cannot round-trip.

Because IREC currently asks for an OpenRocket file, `.ork` export is more valuable than inventing a broad CAD bridge.

### Motor formats

- Parse RASP `.eng` and RockSim `.rse` locally.
- Preserve the original file, normalized samples, digest, source, and license.
- Prefer certification/manufacturer curves over user uploads when several curves exist, but allow comparison.
- Keep the demo database bundled and deterministic.

### RocketPy

Use RocketPy first as an optional external validator, not as the interactive core. A Python subprocess adapter can accept an Ascent run manifest and emit a normalized result JSON. Pin the environment and RocketPy version. This preserves the Rust core while gaining weather, 6-DOF, Monte Carlo, and real-flight comparison capabilities from a validated MIT-licensed project ([RocketPy repository](https://github.com/RocketPy-Team/RocketPy)).

### RASAero II

RASAero II is valuable for transonic/supersonic coefficient and trajectory comparisons, but its official download is Windows-only, depends on .NET 3.5, and lists version 1.0.2.0 from May 2019 ([official download](https://rasaero.com/dl_software_ii.htm)). Its site reports a 3.47% average apogee error across its comparison set, but that is a vendor-maintained comparison and includes some post-flight adjusted cases; treat it as useful evidence, not an independent guarantee ([altitude comparisons](https://rasaero.com/comparisons-alt.htm)).

Do not make RASAero a Mac v0.1 dependency. Add a Windows-side file adapter later if users demand it.

## The fidelity ladder: keep, change, or kill

### Keep: imported aero tables

The safest fidelity boundary is a documented table of coefficients versus Mach and angle of attack, with source and validity metadata. Ascent can consume tables from RASAero, wind-tunnel work, CFD, or team data without pretending to own those methods.

### Change: OpenVSP

Use OpenVSP for parametric geometry exchange and automated trade studies only after a mapping from Ascent components to OpenVSP geometry is proven. Do not label VSPAERO as a rocket transonic fidelity tier without validation.

### Defer heavily: OpenFOAM

A credible bridge eventually needs:

- watertight geometry and a repeatable mesher;
- a narrow solver/model choice;
- boundary-condition templates;
- residual and force convergence checks;
- mesh-refinement evidence;
- validated reference cases;
- provenance for every generated case.

The first honest feature is **Export validated CFD case**, not “one-click accurate CFD.” Running and rendering can follow after the exported cases survive expert review.

### Kill for this product stage: GMAT

GMAT is excellent for orbital mission design but does not advance the target-apogee competition job. Removing it makes Ascent more serious, not less ambitious.

## Testing and engineering bar

### Test suites

1. `domain`: schema migration, units, component invariants, hashes.
2. `motors`: parser corpus, interpolation, impulse, digest, malformed input.
3. `aero`: published equation cases and symmetry/boundary cases.
4. `sim`: analytic trajectories, event order, recovery, convergence.
5. `validation`: golden references and tolerance semantics.
6. `optimize`: feasibility, deterministic ranking, no silent mutation.
7. `interop`: import/export round trips and explicit loss reports.
8. `app`: stale-run rejection, undo/redo, cancellation, file recovery.
9. `e2e`: open preset -> edit -> simulate -> review -> export.

### Performance targets

These are engineering targets to measure, not claims:

- single deterministic run under 50 ms on the demo Mac;
- design edit to scalar feedback under 100 ms;
- viewport at 60 fps while idle and during playback;
- no more than 1,000 plotted samples per series after shape-preserving downsampling;
- cancellation checked between batches/steps;
- a 1,000-run dispersion study under two seconds only after parallel benchmarks justify it.

Add benchmarks for motor interpolation, aero evaluation, one full flight, and batch simulation. Store benchmark history but do not turn the demo into a performance dashboard.

### Failure behavior

Every calculation returns structured warnings/errors. Reject negative mass, impossible geometry, non-monotonic motor time, NaNs, unsupported Mach, impossible event order, and non-convergent runs. Autosave project edits atomically. A crash during simulation must not corrupt the design.

## Licensing and data safety

- OpenRocket is GPLv3. Reading its papers, comparing outputs, and implementing compatible files is different from copying/linking its code. Make the licensing choice explicit before embedding any OpenRocket component.
- RocketPy is MIT-licensed and is easier to use as an optional validator.
- OpenVSP uses NASA Open Source Agreement 1.3, which requires separate review before redistribution.
- ThrustCurve's API specification is ISC-licensed, while individual data files expose their own `PD`, `free`, or `other` license. Store and honor that metadata.
- Do not send proprietary team designs to an AI provider without explicit opt-in. The core application remains offline-capable.

This is engineering guidance, not legal advice; verify redistribution choices before public release.

## Build sequence

### July 16–23: proof artifact, approximately 25 hours

#### Day 1 — trusted motor and vertical-flight kernel

- Rust workspace and domain types.
- One bundled motor curve with provenance.
- Interpolation, impulse, mass depletion, and tests.
- Point-mass derivative and analytic no-drag fixture.

**Exit:** CLI prints a deterministic trajectory and all physics tests pass.

#### Day 2 — flight phases and convergence

- RK4, rail constraint, drag, atmosphere, event detection.
- Rail exit, burnout, apogee, recovery, landing.
- Timestep convergence report.

**Exit:** event timeline and summary are stable at `dt/2`.

#### Day 3 — one external reference

- Build one frozen rocket and motor fixture.
- Compare against manually exported OpenRocket or RocketPy values.
- Record tolerances, model differences, and known assumptions.

**Exit:** evidence page can state what agrees, what differs, and why.

#### Day 4 — parametric vehicle and stability

- Nose/body/fins/payload/motor/chute model.
- Geometry mass, CG, Barrowman CP, time-varying stability.
- IREC constraints and rule-source links.

**Exit:** editing mass or fins produces correct, tested stability changes.

#### Day 5 — application shell and design mode

- Tauri + React shell.
- Rocket viewport, inspector, motor selector.
- design revision, dirty/stale/running/current states.

**Exit:** a person can alter the reference rocket without touching JSON.

#### Day 6 — flight mode

- animated launch playback from a completed result;
- linked telemetry plots and event timeline;
- summary cards and warnings.

**Exit:** full launch story works offline with one click.

#### Day 7 — Flight Review and target solver

- IREC 10K profile;
- constraint panel;
- motor/ballast search;
- before/after diff.

**Exit:** Ascent repairs one deliberately infeasible configuration.

#### Day 8 — provenance, tests, and polish

- evidence drawer, input hash, model/version display;
- fixture regression and end-to-end happy path;
- failure-state UI and demo reset.

**Exit:** clean restart produces the same result without network.

#### Day 9 — demo hardening

- record backup video;
- rehearse 90-second script;
- freeze dependency versions and demo data;
- fix only demo-blocking defects.

### First 30 days

- local ThrustCurve search/cache with license-aware selection;
- RASP and RSE import;
- versioned project folder and immutable run records;
- `.ork` subset import/export and loss reports;
- real PDF Flight Review export;
- measured mass/CG override workflow;
- flight-data CSV import and overlay;
- RocketPy validation adapter;
- parameter sensitivity and uncertainty bands.

### 60–90 days

- full atmosphere/wind profiles;
- Monte Carlo/dispersion with deterministic seeds;
- dual-deploy recovery and configurable events;
- richer drag/stability lookup tables;
- multi-stage domain model;
- real rigid-body 6-DOF only after validation fixtures are ready;
- Git-aware run/config comparison;
- team review/sign-off and requirement traceability.

### Three to six months

- robust multi-objective design optimization;
- verification-gated copilot with typed actions;
- calibration from measured flight data;
- RASAero file adapter;
- CAD/OpenVSP geometry exchange;
- external CFD case export only after a validated narrow case exists;
- rule packs for additional competitions and certification workflows.

## What not to build yet

- arbitrary CAD modeling;
- a generic physics engine;
- orbital trajectories or GMAT integration;
- guidance, controls, HIL, or avionics simulation;
- collaborative cloud backend;
- plugin marketplace;
- generalized agent framework;
- “one-click CFD”;
- a copilot that can edit anything;
- a huge motor database before one curve is handled correctly;
- a claimed 6-DOF solver without validation.

## Tonight's first build task

The first task remains one verified number, but the acceptance criteria are stronger:

1. Create `ascent-domain` and `ascent-sim`.
2. Add one source-controlled C6-class thrust curve with source, license, wet mass, propellant mass, and expected impulse.
3. Write failing tests for curve validation, impulse, interpolation boundaries, and complete mass depletion.
4. Implement a no-drag vertical-flight derivative and RK4.
5. Test the analytic constant-thrust/no-drag case.
6. Add drag and run the chosen reference rocket.
7. Save a machine-readable summary containing apogee, burnout, maximum velocity, timestep, and input hash.

Do not start Tauri until that summary is deterministic and its tests pass.

## Main risks and mitigations

| Risk | Mitigation |
|---|---|
| The demo becomes a prettier OpenRocket | Lead with target-flight solving, rule verification, provenance, and cross-validation. |
| Physics appears sophisticated but is wrong | Narrow the model, name assumptions, validate layers, and expose validity. |
| 3D consumes the schedule | Keep the same interaction functional in a strong 2D side view. |
| Copilot makes it look like a wrapper | Remove it from the critical demo; deterministic optimizer first. |
| Scope expands toward the professional stack | Require a user job and validation plan before every bridge. |
| Cross-engine results disagree | Make disagreement a named result with method/version metadata, not a hidden failure. |
| Motor data changes or carries restrictions | Cache exact curves and preserve source/license/digest. |
| Rules change annually | Version rule packs and cite the governing document/section. |

## Research gaps

- No interviews with current IREC teams were conducted for this report. The competition requirements are primary-source facts; which workflow pain hurts most is still an inference.
- RASAero accuracy figures are vendor-published and should not be treated as independent validation.
- A clean-room `.ork` importer/exporter needs a dedicated conformance study across real files and versions.
- The right Mach-dependent drag model for Ascent's first general-purpose release remains an engineering research task. A narrow demo table is not a universal solution.
- OpenVSP/VSPAERO and OpenFOAM should not be scheduled until a rocket-specific validation case proves useful fidelity for the target users.

## Sources

### User workflow and requirements

1. [IREC 2026 Rules and Requirements](https://www.esrarocket.org/s/IREC-Rules-and-Requirements-Document-2026-v10-f8wf.pdf) — categories, target apogees, scoring, modeling award, and required information.
2. [IREC 2026 Design, Test, and Evaluation Guide](https://www.esrarocket.org/s/2026-IREC-DTEG-V11.pdf) — rail, recovery, stability, testing, and trajectory constraints.
3. [IREC 2026 Progress Report](https://www.esrarocket.org/s/2026-IREC-Editable-Progress-Report-1-11-24-25.pdf) — exact predicted-flight fields and submission conditions.
4. [IREC sample flight card](https://www.esrarocket.org/s/sample_consolidated_flight_card_and_post_flight_record_v11.pdf) — preflight prediction and postflight measurement record.

### Simulation and validation

5. [OpenRocket technical documentation](https://openrocket.sourceforge.net/techdoc.pdf) — aero, atmosphere, 6-DOF simulation, events, architecture, and validation.
6. [OpenRocket repository](https://github.com/openrocket/openrocket) — current implementation, GPL license, examples, core publication, and tests.
7. [OpenRocket feature documentation](https://openrocket.readthedocs.io/en/latest/introduction/features.html) — product capabilities and comparisons.
8. [OpenRocket file-format history](https://github.com/openrocket/openrocket/blob/unstable/fileformat.txt) — format versions and 26.xx development changes.
9. [OpenRocket command-line arguments](https://github.com/openrocket/openrocket/blob/unstable/docs/source/dev_guide/command_line_arguments.rst) — documented CLI surface.
10. [RocketPy repository](https://github.com/RocketPy-Team/RocketPy) — MIT license, models, datasets, and test architecture.
11. [RocketPy validation paper](https://doi.org/10.1061/%28ASCE%29AS.1943-5525.0001331) — published high-power flight-simulation validation.
12. [RocketPy equations of motion](https://github.com/RocketPy-Team/RocketPy/blob/master/docs/technical/equations_of_motion.rst) — rigid-body model reference.
13. [RocketPy Bella Lui acceptance test](https://github.com/RocketPy-Team/RocketPy/blob/master/tests/acceptance/test_bella_lui_rocket.py) — real-flight comparison thresholds.
14. [RocketPy NDRT acceptance test](https://github.com/RocketPy-Team/RocketPy/blob/master/tests/acceptance/test_ndrt_2020_rocket.py) — real-flight comparison thresholds.
15. [ThrustCurve API documentation](https://www.thrustcurve.org/info/api.html) — search/download workflow and units.
16. [ThrustCurve OpenAPI schema](https://www.thrustcurve.org/api/v1/swagger.json) — current request/response fields, samples, and license metadata.
17. [RASAero II official site](https://rasaero.com/home.htm) — model scope and vendor claims.
18. [RASAero II download](https://rasaero.com/dl_software_ii.htm) — version, Windows support, and release history.
19. [RASAero altitude comparisons](https://rasaero.com/comparisons-alt.htm) — vendor-published comparison set and caveats.

### Application and future integrations

20. [Tauri architecture](https://v2.tauri.app/concept/architecture/) — Rust/WebView/process model.
21. [OpenVSP repository](https://github.com/OpenVSP/OpenVSP) — geometry scope, licensing, and build/API surface.
22. [OpenVSP API documentation](https://openvsp.org/api_docs/latest/) — headless automation and language bindings.
23. [OpenFOAM for macOS](https://openfoam.org/download/macos/) — supported Multipass/Ubuntu route.
24. [NASA GMAT repository](https://github.com/nasa/GMAT) — mission-design scope, scripting, APIs, and macOS support.

## Methodology

The research decomposed Ascent into five questions: the concrete user job; required physics and honest model boundaries; validation evidence; desktop/data architecture; and future interoperability. Primary and technical sources were preferred over marketing and forum summaries. Key documents were downloaded and searched in full, including the OpenRocket technical document, current source trees and tests, the IREC 2026 rule/DTEG/report documents, RocketPy equations and acceptance tests, and ThrustCurve's OpenAPI schema. Vendor claims are labeled, recommendations are separated from facts, and unresolved issues are listed above.
