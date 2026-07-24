# Valkyrie 2025 — flight-validation status

**Headline:** Ascent's model has **not** been numerically compared against the
Valkyrie 2025 flight, because the checked-in dataset contains **no simulation
inputs** — only a measured altitude-vs-time trace. A model-versus-flight
comparison is therefore impossible without inventing a vehicle, and the case is
retained at **`FlightDataAvailable`**, not `FlightValidated`.

This is the honest outcome of the "is the dataset sufficient?" gate. It is
published whether the answer is yes or no; here it is no.

## What the dataset is

`data/validation/valkyrie-2025/` contains exactly two files:

- `flightInfo_merged.csv` — a two-column export, `time,altitude`, 12 578 rows
  at 100 Hz, spanning 0 → 125.77 s, copied byte-for-byte from RocketPy and
  hash-pinned (`sha256 0c7808…795bc`, verified by
  `crates/ascent-review/tests/validation_library.rs`).
- `ROCKETPY_LICENSE.txt` — the MIT license for the RocketPy **software**. It
  carries no vehicle, motor, or flight-configuration data.

The measured trace itself is well-formed and characterized by an executed test
(`valkyrie_measured_signal_is_characterized`): apogee **2059.0 m at t ≈ 20.84 s**,
a complete ascent and descent ending near ground (7.8 m at 125.77 s).

## Input/output inventory

To run an authoritative simulation and compare it to the flight, a versioned
input must be constructible from documented flight configuration, and the
measured outputs must be defined without guessing. Status of each item the
comparison would need:

| # | Item | Status | Notes |
|---|---|---|---|
| 1 | Vehicle geometry (length, diameter, fins, nose) | **Missing** | Not present in either file. |
| 2 | Dry mass | **Missing** | — |
| 3 | Propellant mass | **Missing** | — |
| 4 | Motor thrust curve | **Missing** | No `.eng`/RASP curve anywhere in the repo. |
| 5 | Launch angle / rail geometry | **Missing** | — |
| 6 | Weather / wind | **Missing** | — |
| 7 | Sampling timestamps | **Present** | 100 Hz `time` column. |
| 8 | Altitude source / datum | **Partial** | Altitude column present, but datum is under-specified (trace starts at 0.34 m, no stated AGL/MSL reference); altimeter calibration absent. |
| 9 | Deployment / recovery timing | **Missing** | No event channel; deployment instant is not marked. |
| 10 | Measured apogee | **Present (derived)** | 2059.0 m at 20.84 s, extracted from the altitude trace. |
| 11 | Flight-event times (burnout, staging, deploy) | **Missing** | No thrust or event channel; a burnout/deploy instant could only be guessed from curvature. |
| 12 | Uncertainty / sensor limitations | **Missing** | No per-sample covariance, no altimeter calibration or error bounds. |

**Six of six input items (1–6) are absent.** A simulation cannot be run with no
vehicle and no motor, so no apogee/time-to-apogee/max-velocity/burnout/
deployment/flight-time comparison can be executed from this dataset.

## Why the inputs are not reconstructed from RocketPy

The upstream RocketPy repository holds a full Valkyrie rocket definition (masses,
a Mach-dependent drag curve, a motor, and a `valkyrie_flight_sim` notebook).
Those inputs were **deliberately not imported** here, for two reasons:

1. **Provenance.** They are another simulator's model definition, not the
   flight's own measured configuration. Copying them in would make Ascent's
   "input" a restatement of RocketPy's assumptions, and any agreement would test
   Ascent against RocketPy's tuning rather than against the flight.
2. **It would require forbidden calibration.** Ascent's solver uses a single
   constant drag coefficient; RocketPy's Valkyrie relies on a Mach-dependent
   drag curve. Reproducing a 2059 m apogee with a constant Cd would require
   fitting that Cd to this flight — precisely the tuning the mandate prohibits
   unless it is declared a calibration case. It has not been, so it is not done.

## What would move this case to `FlightValidated`

A future promotion requires all of:

1. A documented, versioned input for the Valkyrie built from **its own** flight
   configuration (masses, motor thrust curve, geometry, launch angle, site/wind)
   — hash-pinned, with each field's provenance recorded.
2. An authoritative Ascent run producing simulated apogee and time-to-apogee.
3. An executed comparison (a `ComparisonArtifact`) with tolerances fixed
   **before** the result is seen, bound to the case by `case_hash`, that
   **passes** — see [`EVIDENCE_LADDER.md`](EVIDENCE_LADDER.md) for the enforced
   rule and [`SIXDOF_DERIVATION.md`](SIXDOF_DERIVATION.md) for the model's
   documented limits (which may themselves make a real-flight pass unattainable
   without a richer drag model).

Until then the case stays at `FlightDataAvailable`: the measured data is real,
license-clear, hash-pinned, and ingested — and nothing about model accuracy is
claimed.
