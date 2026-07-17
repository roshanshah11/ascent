//! Barrowman center-of-pressure estimation, per the OpenRocket technical
//! documentation (openrocket.sourceforge.net/techdoc.pdf §3) and Barrowman's
//! original method. Validity: small angle of attack, subsonic.
//!
//! Normal-force coefficient derivatives (CNα) are per radian, referenced to
//! the body cross-section area. CP stations are meters aft of the nose tip.

use crate::vehicle::Vehicle;

/// Nose cone shapes supported in v0.1. CNα = 2 for any nose; only the CP
/// station differs by shape (conical 2/3·L, ogive ≈ 0.466·L).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoseShape {
    Conical,
    Ogive,
}

pub struct ComponentCn {
    pub cn_alpha: f64,
    pub cp_from_nose_m: f64,
}

/// Nose cone: CNα = 2 (slender-body result, independent of shape).
pub fn nose_cn(shape: NoseShape, length_m: f64) -> ComponentCn {
    let frac = match shape {
        NoseShape::Conical => 2.0 / 3.0,
        NoseShape::Ogive => 0.466,
    };
    ComponentCn {
        cn_alpha: 2.0,
        cp_from_nose_m: frac * length_m,
    }
}

/// Trapezoidal fin set of N fins (valid for N = 3 or 4) including the
/// body-interference factor K = 1 + r/(s + r).
///
/// CNα_fins = K · [4N(s/d)²] / [1 + √(1 + (2ℓ/(c_r+c_t))²)]
/// where ℓ is the mid-chord line length.
///
/// CP aft of the fin root leading edge:
/// x̄ = (m/3)·(c_r + 2c_t)/(c_r + c_t)
///   + (1/6)·(c_r + c_t − c_r·c_t/(c_r + c_t))
pub fn fin_set_cn(v: &Vehicle) -> ComponentCn {
    let f = &v.fins;
    let (cr, ct, s, m) = (f.root_chord_m, f.tip_chord_m, f.span_m, f.sweep_m);
    let d = v.diameter_m();
    let r = d / 2.0;
    let n = f.count as f64;

    // Mid-chord line: from root mid-chord to tip mid-chord.
    let mid_chord_offset = m + ct / 2.0 - cr / 2.0;
    let l = (s * s + mid_chord_offset * mid_chord_offset).sqrt();

    let interference = 1.0 + r / (s + r);
    let cn_alpha = interference * (4.0 * n * (s / d) * (s / d))
        / (1.0 + (1.0 + (2.0 * l / (cr + ct)) * (2.0 * l / (cr + ct))).sqrt());

    let xf = m / 3.0 * (cr + 2.0 * ct) / (cr + ct)
        + (cr + ct - cr * ct / (cr + ct)) / 6.0;

    ComponentCn {
        cn_alpha,
        cp_from_nose_m: f.position_from_nose_m + xf,
    }
}

/// Total CP: CNα-weighted combination of component CPs.
/// v0.1 vehicles have a straight body (no transitions), so the body tube
/// contributes no normal force in the Barrowman model.
pub fn total_cp_from_nose_m(v: &Vehicle, nose_shape: NoseShape) -> f64 {
    let nose = nose_cn(nose_shape, v.nose.length_m);
    let fins = fin_set_cn(v);
    let total_cn = nose.cn_alpha + fins.cn_alpha;
    (nose.cn_alpha * nose.cp_from_nose_m + fins.cn_alpha * fins.cp_from_nose_m) / total_cn
}
