# Reduced Rotational Flight Solver Contract

This document fixes the equations, coordinate conventions, event semantics,
determinism rules, and evidence envelope for Ascent's `SixDof` solver. The
name is heritage: the solver integrates an attitude, but its rotational
dynamics are deliberately reduced, so "6-DOF" here labels the tier, not a
fidelity claim. All internal quantities are SI.

The integrated state is a 13-element rigid-body vector, but the rotational
response is intentionally limited to a pitch/yaw restoring torque about the
static margin. The model does **not** include:

- roll dynamics or fin cant (the roll-rate derivative is never written);
- a full inertia tensor — one scalar pitch/yaw inertia is used, so there is no
  inertia-tensor cross-coupling between axes;
- gyroscopic cross-coupling between body axes;
- aerodynamic rotational damping (no pitch/yaw damping derivatives).

The attitude is also **frozen during descent**: the rotational block is
integrated only in the ascent phase. Every attitude and angular-rate output is
therefore a reduced-model estimate for ascent, not a validated
six-degree-of-freedom prediction. See [`docs/EVIDENCE_LADDER.md`](EVIDENCE_LADDER.md)
for the evidence backing these outputs.

## State and coordinate conventions

The coupled state is

`y = [r_i, v_i, q_bi, omega_b]`,

where `r_i = [x, y, z]` is inertial position in metres, `v_i` is inertial
velocity in metres per second, `q_bi = [w, x, y, z]` is a unit quaternion, and
`omega_b = [omega_x, omega_y, omega_z]` is body angular rate in radians per
second. Quaternion components are scalar-first.

The inertial frame is right-handed and fixed to the launch site: `+z` is up,
`+x` is nominal downrange, and `+y` completes `x cross y = z`. The body frame
is right-handed: `+z_b` is the rocket longitudinal axis toward the nose,
`+x_b` lies in the nominal downrange pitch plane, and `+y_b` completes
`x_b cross y_b = z_b`. At a vertical zero-azimuth launch the body and inertial
axes coincide.

`q_bi` is an active body-to-inertial rotation: an inertial vector is
`a_i = q_bi * [0, a_b] * conjugate(q_bi)`. Products use the Hamilton
convention. Body angular rate therefore gives

`q_dot = 0.5 * q_bi * [0, omega_b]`.

Launch tilt is measured in radians away from inertial `+z`. Positive tilt is
toward the configured azimuth, measured counter-clockwise from inertial `+x`
toward `+y` when viewed from above. The launch quaternion is the shortest
rotation carrying `+z_b` onto
`[sin(tilt) cos(azimuth), sin(tilt) sin(azimuth), cos(tilt)]` with zero roll.

The quaternion participates in every RK4 stage. After every accepted full
step and every interpolated event state it is explicitly divided by its
Euclidean norm. A zero or non-finite norm is an error. The evidence tolerance
is `abs(norm(q_bi) - 1) <= 1e-12` for every recorded sample.

## Coupled fixed-step integration

Free-flight translation and rotation use classical fixed-step RK4:

`y[n+1] = y[n] + dt/6 * (k1 + 2 k2 + 2 k3 + k4)`,

with the usual half-step stage states. Position, velocity, quaternion, and
angular rate are advanced as one coupled state. `dt` is never adapted. Event
crossings use deterministic linear interpolation within the accepted step and
do not introduce a variable substep. Configuration rejects non-finite values,
non-positive timestep or inertia/area, negative aerodynamic derivatives, and
non-positive hard time limits before integration.

## Forces and moments

At time `t`, total mass is dry mass plus `motor.mass_at(t)`. Thrust acts along
`+z_b`. Gravity is `mass * [0, 0, -g]_i`.

The wind profile is piecewise constant in altitude and supplies a full
three-dimensional inertial velocity. Relative air velocity is
`v_air_i = v_i - wind_i(z)` and is rotated into the body frame. The rocket's
constant body `Cd*A` produces axial drag opposite relative airflow. During
free ascent, Barrowman normal force uses the component of relative airflow
perpendicular to `+z_b`:

`F_normal_b = -0.5 * rho * |v_air|^2 * A_ref * CN_alpha * alpha_vector`,

where `v_lateral` is the body-frame airflow perpendicular to `+z_b`,
`alpha = atan2(|v_lateral|, v_axial)`, and `alpha_vector` has magnitude
`alpha` in the direction of `v_lateral`. Thus the normal-force magnitude is
`0.5 * rho * |v_air|^2 * A_ref * CN_alpha * alpha`, applied opposite the
transverse airflow direction. This corrects the earlier ambiguous
transverse/axial wording, which could be read as an erroneous `tan(alpha)`
force law. The model remains limited to the documented small-angle validity
envelope. The force is perpendicular to the body axis and opposes angle of
attack. Its restoring pitch/yaw moment is the cross product from CG to CP,
with lever arm `(CP - CG)` along the aft direction. Pitch and yaw share the
configured longitudinal moment of inertia. The axial roll moment and
`omega_z` are constrained to zero because roll dynamics are outside this
tier.

In recovery descent, the existing body-plus-parachute `Cd*A` model acts
opposite the three-dimensional wind-relative velocity. Attitude is frozen at
the normalized apogee attitude and all angular rates are zero; recovery
attitude is bounded state, not a validated parachute-orientation prediction.

## Vehicle-tree boundary

`ascent-sim` does not depend on `ascent-aero`. The caller flattens the Step 1/3
vehicle tree into immutable `SixDofVehicle` parameters before constructing the
engine:

- dry CG and pitch/yaw MOI come from `Vehicle::mass_properties()`;
- CP comes from `ascent_aero::total_cp_from_nose_m` on
  `ascent_aero::Vehicle::from_tree`;
- `CN_alpha` is the sum of the tree-derived nose and fin Barrowman terms;
- reference area is the body cross-section derived from the tree diameter.

The engine accepts these injected values and never invents fallback geometry.
The configured dry CG is the current rigid-body approximation; motor mass
changes total mass through the burn, while motor-induced CG/MOI migration is
not claimed as validated by this tier.

## Launcher and event semantics

The initial phase is `Pad`. The vehicle is held while thrust at both ends of a
step does not exceed the component of weight opposing motion along the
configured rail. `Liftoff` occurs at the first released step boundary.

During `Rail`, position and velocity are constrained to the configured 3D rail
axis, attitude is fixed to the launch attitude, and angular rate is zero. Only
the along-axis components of thrust, gravity, and axial aerodynamic drag
advance rail distance. `RailExit` is the interpolated first crossing of the
configured physical rail length, after which unconstrained coupled motion
begins.

`Burnout` is recorded at `motor.burn_time()` by interpolation in the first
step spanning that time. `Apogee` is the interpolated upward-to-non-upward
crossing of inertial vertical velocity. If recovery exists,
`RecoveryDeploy` is coincident with apogee and starts `Descent`. `Landing` is
the interpolated positive-to-non-positive inertial `z` crossing; its recorded
altitude is exactly zero and simulation terminates. A motor that never
overcomes the rail-projected weight produces no liftoff event. The detailed
history exposes time, position, velocity, quaternion, angular rate, and phase
(`Pad`, `Rail`, `Ascent`, `Descent`, or `Grounded`).

## Determinism and input identity

`SixDofEngine` is immutable after construction. Identical inputs use the same
fixed operation order and produce byte-identical JSON histories and standard
`SimSummary` values. Its SHA-256 input identity covers the canonical serialized
`Rocket`, `Motor`, `Environment`, and `SimConfig`, plus every configured
six-DOF field:

- rigid-body/aero parameters (CG, pitch/yaw MOI, CP, `CN_alpha`, area);
- every 3D wind layer (altitude and all three velocity components);
- launch tilt, launch azimuth, and launcher configuration.

Changing any one of those categories must change the hash.

## Validity envelope and residual gates

The evidence fixtures are deterministic, constant-`Cd`, incompressible,
subsonic flights below Mach 0.8 with positive static margin, small angle of
attack, no fin cant, and no roll excitation. The named acceptance gates are
fixed before measurement:

| Fixture | Quantity | Acceptance tolerance |
|---|---|---:|
| Calm vertical | apogee vs native golden pin | relative residual `<= 1e-6` |
| Planar wind | apogee vs `simulate_planar` | relative residual `<= 2%` |
| Planar wind | landing/downrange vs `simulate_planar` | absolute residual `<= 5 m` and relative residual `<= 5%` |
| Planar wind | pitch/weathercock measure vs `simulate_planar` | absolute residual `<= 2 deg` |
| Tilted rail | quaternion norm | absolute norm error `<= 1e-12` |
| Tilted rail | attitude after rail exit | finite, tilt bounded to `[0, 45] deg`, roll component bounded to `1e-10 rad` |
| Tilted rail | trajectory | finite, positive apogee, and ground contact after apogee |

Signed, absolute, and relative residuals for the final calm and planar fixtures
are recorded in `docs/EVIDENCE.md` after implementation.

Within those tested calm/planar subsonic fixtures, apogee, downrange, and
pitch/weathercock quantities may be labeled `Validated`. Outside that envelope
they are `Extrapolated`. Roll, crossrange, fin-cant-sensitive quantities, and
recovery attitude are always `Extrapolated`: roll dynamics and fin cant are
explicitly unsupported, and no output sensitive to them may be presented as
validated.
