//! Stability margin over the burn: (CP − CG(t)) / reference diameter.

use crate::barrowman::{total_cp_from_nose_m, NoseShape};
use crate::vehicle::Vehicle;
use ascent_domain::Motor;

/// Static stability margin in calibers at burn time `t`.
/// Positive = CP aft of CG = stable. CP is attitude-independent in the
/// Barrowman small-angle model, so only CG moves during the burn.
pub fn stability_calibers_at(v: &Vehicle, nose_shape: NoseShape, motor: &Motor, t: f64) -> f64 {
    let cp = total_cp_from_nose_m(v, nose_shape);
    let cg = v.cg_at(motor, t);
    (cp - cg) / v.diameter_m()
}

/// Stability margin as percent of total rocket length at burn time `t` —
/// the form IREC 2026 states its bands in (min 7.5% subsonic, max 18% at
/// launch / 25% through flight).
pub fn stability_pct_of_length_at(
    v: &Vehicle,
    nose_shape: NoseShape,
    motor: &Motor,
    t: f64,
) -> f64 {
    let cp = total_cp_from_nose_m(v, nose_shape);
    let cg = v.cg_at(motor, t);
    (cp - cg) / v.length_m() * 100.0
}

/// Margin at ignition and at burnout — the two extremes that matter for
/// rules checks (motors at the rear: burnout margin ≥ ignition margin).
pub fn stability_envelope(
    v: &Vehicle,
    nose_shape: NoseShape,
    motor: &Motor,
) -> (f64, f64) {
    (
        stability_calibers_at(v, nose_shape, motor, 0.0),
        stability_calibers_at(v, nose_shape, motor, motor.burn_time()),
    )
}
