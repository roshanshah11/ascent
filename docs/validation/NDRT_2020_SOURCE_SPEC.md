# NDRT 2020 — Source Specification

Frozen provenance for the Notre Dame Rocketry Team (NDRT) 2020 full-scale
flight comparison. This document pins every source, states its units and the
conversions applied, records the ambiguities encountered, and fixes the
**no-fitting rule** that governs the comparison.

## Upstream source

- **Repository:** `RocketPy-Team/RocketPaper`
- **Commit:** `23d9271db533f53eb91c778f67992c0ccbb0fd0f`
- **Path:** `NDRT_2020_launch_vehicle/`
- **License:** MIT — © 2020 Projeto Jupiter (checked in as
  `data/validation/ndrt-2020/ROCKETPAPER_LICENSE.txt`)

All files are stored **byte-for-byte unchanged** under
`data/validation/ndrt-2020/`.

## Hash-pinned files (SHA-256)

| File | SHA-256 | Role |
|------|---------|------|
| `fullscale_2-23_raven1_crop.csv` | `1cc15862ddb3367d09225037cfe8dd1f68687d1d2292ef7ecd5f2a344736d3f6` | Measured RAVEN altimeter telemetry (the comparison fixture) |
| `Cesaroni_4895L1395-P.eng` | `d60b9e6f8d4bb4621a58e98873f69bfa8ef49efe4b0f78fdf345aa52c50b47ce` | RASP thrust curve (motor input) |
| `NDRT_ROCKETPY.ipynb` | `e01c71b8f9307f94a66a61ab27dbb9fbc7140f2f26af5795035d4dde1f752153` | Upstream RocketPy analysis (units / apogee-row source) |
| `README.md` | `bfe65d7f28186bad1607afbd83d25949f81aa54d0dc968a4703727b950d38615` | Channel description; states apogee is at CSV row 343 |
| `env_23.nc` | `c6c378c762b3bb5200f604d4c176ab24c66a4e040531acb194d7efbc217d07c2` | ERA5 atmosphere sounding (**not consumed** — see Atmosphere) |
| `ROCKETPAPER_LICENSE.txt` | `37954555faa0cd7bec1fc4b05fd51cc40abe97294c387d0b1890019cbf76ec3b` | Upstream MIT license |

The comparison consumes the **CSV** (measured) and the **`.eng`** (motor input).
The notebook and README are provenance/units references; `env_23.nc` is
retained for provenance but cannot be read by Ascent.

## Telemetry structure (`fullscale_2-23_raven1_crop.csv`)

Header (verbatim, leading spaces intact):

```
Time-Axial Accel (Gs), Axial Accel (Gs), bILBA, Time (s), Altitude (Ft-AGL), bILBA
```

- **1819 data rows**, 6 columns each. Parsed with rows, columns, order, and raw
  values preserved; no cell is edited.
- The file packs **two independent time series** column-wise, each with its own
  clock:
  - **Axial acceleration channel** — columns 0 (`Time-Axial Accel (Gs)`) and 1
    (`Axial Accel (Gs)`). Cadence 0.0025 s, span 0.0–4.546 s.
  - **Barometric altitude channel** — columns 3 (`Time (s)`) and 4 (`Altitude
    (Ft-AGL)`). Cadence 0.05 s, span 0.044–90.945 s.
- Columns 2 and 5 (`bILBA`) are status flags; the README marks them "useless".
- Both channels are strictly time-increasing with no missing rows. Cadence is
  uniform apart from a single 0.00375 s step (accel) and a single 0.05125 s step
  (altitude). Altitude values are quantized to ~0.34 m barometric steps.

Only the **altitude channel** drives the comparison (altitude AGL vs. time
through apogee).

## Units and conversions

| Quantity | Source unit | Normalized unit | Conversion |
|----------|-------------|-----------------|------------|
| Time | seconds | seconds | none |
| Altitude | feet AGL | metres AGL | `ft / 3.28084` (the **source notebook's** exact factor) |
| Axial acceleration | g | g | none |

## Launch alignment (deterministic, source-only)

Launch is aligned to the **start of the recording** — the altitude channel's
own `Time (s)` at t = 0 — with **no offset applied**. This is exactly how the
upstream notebook reports measured time-to-apogee (17.095 s) against the
ignition-referenced RocketPy clock. The apogee row is the **argmax of the
altitude channel** (ties → first), which the README independently pins at file
row 343 (0-based data index 341). The alignment is a pure function of the
telemetry: no simulation output enters it, and it is never chosen to minimize
error.

## Input mapping (frozen, from the goal input block + NDRT FRR)

| Input | Value | Ascent target |
|-------|-------|---------------|
| Airframe dry mass | 18.998 kg | `Rocket::dry_mass_kg` |
| Motor loaded / dry / propellant | 4.323 / 1.848 / 2.475 kg | imported `Motor` (`.eng`) |
| Lift-off mass | 23.321 kg | `dry_mass_kg + motor.total_mass_kg` |
| Body radius | 0.1015 m | reference area `π·r²` |
| Drag coefficient | 0.44 (constant) | `DragModel::cd` |
| Rail length | 3.353 m | `Environment::rail_length_m` |
| Gravity | 9.80665 m/s² | `Environment::gravity_ms2` |
| Motor | Cesaroni 4895L1395-P | `parse_eng` of the `.eng` |

Engine: **`simulate_vertical`** (RK4 1-DOF vertical) — the authoritative
altitude-vs-time engine, faithful to the point-mass inputs supplied (single Cd,
single reference area, motor curve, standard atmosphere) and to the near-vertical
launch (inclination 90°).

## Atmosphere limitation (declared, not fitted)

The source simulation used the ERA5 reanalysis sounding `env_23.nc` (NetCDF).
**Ascent cannot consume NetCDF soundings**, so the comparison runs on the **1976
US Standard Atmosphere**. This is a genuine model-input mismatch, documented
here and in the comparison report — it is **not** approximated, back-fitted, or
tuned. Two secondary consequences are recorded rather than corrected:

1. The standard atmosphere is referenced to sea level; the 206 m launch
   elevation density offset is not applied.
2. Barometric AGL altitude is compared directly to the simulator's AGL output.

## Ambiguities / conflicts recorded

- **Burn time.** The `.eng` terminal-zero sample is at **3.45 s**; the goal
  input block lists **3.433 s**. The imported curve is used **verbatim**
  (burn time = 3.45 s). The discrepancy is recorded as a known mismatch, not
  reconciled by editing the source.
- **Measured apogee value.** Notebook cell 20 reports **4,320 ft = 1316.736 m**,
  while the raw channel maximum (`max(ft/3.28084)`) is **≈ 1320.357 m**. The
  comparison value is the **raw telemetry maximum**. Reason: it is a
  deterministic derivation from the pinned CSV (argmax of the altitude channel),
  reproducible from the fixture hash without adopting a rounded, transcribed
  figure. The result is unchanged by the choice — both values are far inside the
  10 % apogee tolerance.

## No-fitting rule

The comparison is **executed and non-calibrated** — with a bounded independence
claim:

- **Ascent performed no fitting or tuning against the measured flight.**
  However, the independence of the externally supplied NDRT parameters,
  especially `Cd = 0.44`, from the measured flight has **not** been established.
  The values are reused as the team published them; whether `Cd` was itself
  derived against this flight is unknown. This is therefore **not** a blind,
  independent, or preflight validation.
- No parameter is fit to measured output. Cd, mass, thrust, wind, launch angle,
  and time offset are all fixed from the sources **before** any result is seen.
- The pass/fail tolerances (apogee ≤ 10 %, time-to-apogee ≤ 10 %, normalized
  altitude RMSE ≤ 10 %, burnout ≤ 1 timestep) are **frozen** in advance and are
  the acceptance bar, not knobs.
- Measured metrics are **derived** from the hash-pinned telemetry at run time,
  never hardcoded.
- Burnout is an **input-consistency** check only; velocity, acceleration,
  descent, recovery, and landing are excluded from pass/fail.
- If an input cannot be represented faithfully (e.g. the ERA5 atmosphere), the
  mismatch is **documented**, not approximated.
