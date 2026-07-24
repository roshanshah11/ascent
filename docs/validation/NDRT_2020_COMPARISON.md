# NDRT 2020 — Executed Comparison

Executed comparison of Ascent's authoritative vertical engine
(`simulate_vertical`, RK4 1-DOF) against the measured NDRT 2020 full-scale
flight. Scope: **launch through measured apogee only**. Provenance, units, and
the no-fitting rule are frozen in
[`NDRT_2020_SOURCE_SPEC.md`](./NDRT_2020_SOURCE_SPEC.md).

## Credibility scope

**Ascent performed no fitting or tuning against the measured flight.** However,
the independence of the externally supplied NDRT parameters, especially
`Cd = 0.44`, from the measured flight has **not** been established. The vehicle
mass, drag coefficient, reference area, motor curve, atmosphere, and launch
alignment are all fixed from the sources before any result is seen — but those
sources are the team's own published values, and whether `Cd = 0.44` was itself
derived against this flight is unknown.

This is therefore **not** a blind, independent, or preflight validation. It is
an executed, non-calibrated comparison whose independence guarantee stops at
"Ascent did no fitting," not at "the inputs are provably flight-independent."

The comparison is produced by
`ascent_review::ndrt_2020::run_comparison`, which parses the hash-pinned
telemetry, imports the motor, runs the simulator, interpolates the simulated
altitude onto the measured timestamps, and **recomputes every error and status**
into a `ComparisonArtifact`. It is exercised by
`crates/ascent-review/tests/ndrt_2020.rs`.

## Identity

| Field | Value |
|-------|-------|
| Case id | `ndrt-2020-flight` |
| Case hash (spec) | `5bbb4f61cb064cba0cc1bddd7dbab823e20760f5a1a95389663ed91625f0469c` |
| Model version | `ascent-sim/0.6.0 simulate_vertical (RK4 1-DOF vertical)` |
| Config / input hash | `7a764a79947116c5d3c5609ac974d3d15311d473993a173c1f713e2902cfcd8a` |
| Timestep | 0.005 s |
| Compared samples (launch→apogee) | 342 |

## Measured vs. simulated

| Quantity | Measured | Simulated |
|----------|----------|-----------|
| Apogee (AGL) | 1320.357 m | 1322.284 m |
| Time to apogee | 17.095 s | 16.778 s |
| Burnout | 3.450 s (imported motor) | 3.450 s |
| Altitude RMSE (launch→apogee) | — | 50.025 m |

Measured apogee and time-to-apogee are **derived** from the RAVEN altitude
channel (argmax of `ft/3.28084`, and its own timestamp) — they reconcile with
the upstream RocketPy analysis (1320.357 m, 17.095 s; README apogee row 343).

### Apogee value — which number, and why

The comparison uses the **raw telemetry maximum**, not a printed figure:

| Source | Apogee |
|--------|--------|
| Reported (notebook cell 20) | 4,320 ft = **1,316.736 m** |
| Raw telemetry maximum (`max(ft/3.28084)`) | **≈ 1,320.357 m** |
| **Comparison value** | **raw telemetry maximum (1,320.357 m)** |

Reason: the raw maximum is a **deterministic derivation from the pinned CSV**
(argmax of the altitude channel), reproducible from the SHA-256-pinned fixture
without adopting a rounded, human-transcribed figure. The result is not changed
by this choice; both values sit far inside the 10 % apogee tolerance.

## Metrics — pass/fail

Tolerances were fixed before any result was viewed. The **overall result is
derived from the three primary flight metrics only**; burnout is an
input-consistency check and never gates the flight-validation pass/fail.

**Primary flight metrics — 3/3 pass:**

| Metric | Value | Tolerance | Result |
|--------|-------|-----------|--------|
| Apogee relative error | 0.15 % (abs 1.93 m) | ≤ 10 % | **PASS** |
| Time-to-apogee relative error | 1.86 % (abs 0.317 s) | ≤ 10 % | **PASS** |
| Normalized altitude RMSE | 3.79 % (50.0 m / 1320.4 m) | ≤ 10 % | **PASS** |

**Input-consistency checks — 1/1 pass:**

| Check | Value | Tolerance | Result |
|-------|-------|-----------|--------|
| Burnout difference (sim vs. imported `.eng`) | 0.0 timesteps | ≤ 1 timestep | **PASS** |

**Overall: PASS (3/3 primary metrics).** Burnout consistency is reported but
excluded — it cannot alone force a pass or a fail. Velocity, acceleration,
descent, recovery, and landing are likewise excluded. This exclusion is
enforced in code (`MetricKind::InputConsistency`) and proven by
`burnout_is_not_counted_as_a_flight_validation_metric`.

## Residual diagnosis

The residuals are small and inside every tolerance. Their **causes are not
proven**; several factors plausibly contribute, and constant-Cd behavior is a
leading hypothesis rather than an established cause.

- **Apogee (+1.93 m, +0.15 %).** Near-exact overshoot. For reference, upstream
  RocketPy reported ~1310 m (−0.76 %) against the same flight, so Ascent's
  apogee agreement is comparable.
- **Time to apogee (−0.317 s, −1.86 %).** Ascent reaches apogee marginally
  early.
- **Altitude RMSE (50 m, 3.8 % of apogee).** Concentrated in mid-ascent; the
  endpoints (pad and apogee) agree closely.

**Possible contributors to the residuals (not ranked as proven):**

1. Constant-Cd limitations near transonic conditions (a leading hypothesis for
   the mid-ascent RMSE, not a demonstrated cause).
2. Standard atmosphere substituted for the measured ERA5 sounding.
3. Wind omitted entirely by the 1-DOF vertical model.
4. Uncertain independence of the supplied `Cd = 0.44` from this flight.
5. Telemetry or timing limitations (barometric quantization ~0.34 m, separate
   channel clocks, no ignition-time offset).

No claim is made that constant Cd *caused* the RMSE; isolating the contributions
would require a controlled study that this comparison does not perform.

## Atmosphere / fidelity limits

- Ascent uses the **1976 US Standard Atmosphere**; the source ERA5 sounding
  `env_23.nc` is not consumable and is **not** approximated.
- Standard atmosphere referenced to sea level; the 206 m launch-elevation
  density offset is not applied.
- Motor burn time is the `.eng` terminal-zero (3.45 s); the input block's
  3.433 s is recorded as a known mismatch and the curve is used verbatim.

## Evidence attachment and label

The executed comparison artifact **is attached** to the checked-in case
(`data/validation/cases/ndrt-2020-flight.json`), bound to the case spec by
`case_hash`, internally valid, and passing on its three primary metrics. The
existing schema permits an artifact at any rung — only `FlightValidated`
*requires* one — so attaching it at a lower rung is supported and does not
promote the label.

The case remains at **`FlightDataAvailable`**. Attaching a passing artifact does
**not** auto-grant `FlightValidated`: the label is a declared field, and the
promotion invariant (`enforce_evidence_policy`) only *raises* the bar for a
`FlightValidated` case — it never elevates a case on the strength of an attached
artifact. This is proven by `attached_artifact_matches_a_fresh_run_and_does_not_promote`
and `completion_does_not_auto_grant_flight_validated`.

**Recommendation:** the result would support promotion to `FlightValidated`,
but promotion should wait on human review of (a) the standard-atmosphere
substitution for the ERA5 sounding, (b) the 3.45 s vs. 3.433 s burn-time
provenance, and — new — (c) **the unverified independence of `Cd = 0.44` from
the measured flight.** Until (c) is resolved, this should be described as an
executed non-calibrated comparison, not an independent or blind validation.
Promotion, if approved, means only raising the declared label; the artifact is
already attached.
