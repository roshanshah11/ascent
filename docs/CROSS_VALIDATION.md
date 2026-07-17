# Optional RocketPy cross-validation

The RocketPy bridge is a **comparison seam**, not a second authoritative
solver. Ascent always computes and retains its native deterministic result.
When the optional bridge is available, the app displays native and RocketPy
apogees side by side and reports their absolute delta. It never averages,
substitutes, or feeds a RocketPy result back into native execution.

## Enabling the developer bridge

The default build has no Python/RocketPy dependency and lists only
`ascent-native`. A developer can compile the comparison seam with:

```sh
cargo test -p ascent-sim --features bridge-rocketpy
cargo check -p ascent-app --features bridge-rocketpy
```

`RocketPyEngine` defaults to `python3` plus the checked-in
`bridges/rocketpy/run_ascent.py`; override either with
`ASCENT_ROCKETPY_PYTHON` or `ASCENT_ROCKETPY_SCRIPT`. The `run_spread` Tauri
IPC always returns the native summary. Its `rocketpy` entry is either a
separate summary with `apogee_spread_m`, or an explicit unavailable/error
reason. Expected graceful-unavailable cases are: the feature is not compiled,
the Python executable or bridge script is missing, RocketPy is missing, the
bridge exits nonzero, its JSON is malformed, or its response schema is not
supported.

## Versioned adapter contract

The bridge accepts exactly one stdin JSON request with schema
`ascent-rocketpy-request-v1` and emits exactly one stdout JSON response with
schema `ascent-rocketpy-response-v1`. Diagnostics go only to stderr and
failure exits nonzero. The Rust side preserves the native canonical input hash
instead of accepting one from Python.

| Ascent field (SI) | RocketPy counterpart | Mapping and limitation |
|---|---|---|
| `Rocket.name` | Evidence label only | It does not alter RocketPy physics. |
| `dry_mass_kg` (kg) | `Rocket.mass` (kg) | Direct. |
| drag `reference_area_m2` (m²) | `Rocket.radius` (m) | Radius is `sqrt(area/pi)`, assuming a circular frontal area. |
| drag `cd` | `power_on_drag`, `power_off_drag` | Direct constant drag coefficient; no Mach/Reynolds/angle-of-attack model is claimed. |
| recovery `chute_cd × chute_area_m2` (m²) | `Rocket.add_parachute(cd_s=...)` | Direct CdA, trigger fixed to RocketPy `apogee`, zero lag/noise. `None` means no parachute. |
| motor `thrust_curve` (s, N) | `GenericMotor.thrust_source` | Direct sampled curve plus explicit `(0, 0)` point. `GenericMotor`, not `SolidMotor`, is used because Ascent supplies no grain geometry. |
| motor total/propellant mass (kg) | `GenericMotor.dry_mass`, `propellant_initial_mass` | Direct mass split. The generic motor chamber/nozzle values are fixed numerical placeholders and are not physical claims. |
| gravity (m/s²) | `Environment(gravity=...)` | Direct. |
| `AtmosphereModel::Standard` | custom pressure/temperature table | Generated from Ascent's stated troposphere equation every 100 m from 0 to 11 km. No forecast, reanalysis, URL, current time, wind, or random source is used. |
| `AtmosphereModel::ConstantDensity` (kg/m³) | custom constant pressure/temperature table | Held at 288.15 K with pressure derived from `rho × R × T`; no vertical density variation is claimed. |
| rail length (m) | `Flight.rail_length` | Direct. Launch is fixed vertical (90°), zero wind, 3-DOF. |
| config `max_time_s`, `dt_s` (s) | `Flight.max_time`, `max_time_step` | Direct limits; RocketPy's adaptive integrator is not Ascent's fixed-step RK4. |
| Ascent/RocketPy apogee, burnout, max/rail-exit/landing velocity and time (m, m/s, s) | `Flight` properties and sampled event state | Returned individually in the versioned response. RocketPy's output record is never written into the native summary. |

The current Ascent input lacks body length, nose/fin geometry, CG/CP,
aerodynamic-surface data, wind, launch-site geography, and motor grain
geometry. The bridge therefore runs a deliberately limited vertical point-mass
comparison; it must not be interpreted as a complete RocketPy vehicle model.

## Frozen fixture

`bridges/rocketpy/reference_input.json` is the named deterministic reference:
an Estes Alpha III model (34 g dry), a C6 sampled thrust curve, 25 mm circular
reference area, constant Cd 0.60, 30 cm Cd 0.75 parachute, 0.9 m rail,
standard atmosphere, and a 5 ms maximum time step.

`bridges/rocketpy/reference_output.json` was generated once with **RocketPy
1.12.1** under **Python 3.14.6**, using only the supplied input and the custom
atmosphere above. The generation invocation was:

```sh
PYTHONPATH=/private/tmp/ascent-rocketpy-fixture python3 bridges/rocketpy/run_ascent.py \
  < bridges/rocketpy/reference_input.json
```

It records a 361.09734484916976 m apogee. The feature-gated Rust regression
parses that frozen response and permits at most **0.5 m** apogee deviation,
which is intentionally wider than the frozen JSON's exact replay and narrow
enough to reveal a material bridge/model change.

Known differences remain expected: Ascent uses a one-dimensional fixed-step
RK4 integrator with interpolated crossings; RocketPy uses adaptive integration
and 3-DOF state. RocketPy's rail-exit convention, parachute event handling,
and generic-motor bookkeeping also differ. A delta is diagnostic evidence, not
an error correction or an accuracy ranking.
