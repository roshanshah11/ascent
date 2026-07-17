//! Parametric vehicle model: geometry, component masses, and center of
//! gravity — including CG as a function of time while the motor burns.
//!
//! Positions are measured in meters aft of the nose tip (the datum).
//! Component-internal CG locations use thin-shell / uniform-sheet
//! assumptions, stated per component. Barrowman CP lands in a separate
//! module once the hand-computed fixture (docs/BARROWMAN_WORKSHEET.md)
//! exists; mass properties here are independent of it.

use ascent_domain::Motor;
use serde::{Deserialize, Serialize};

/// Nose cone modeled as a thin conical shell: CG at 2/3 of its length
/// aft of the tip.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoseCone {
    pub length_m: f64,
    pub base_diameter_m: f64,
    pub mass_kg: f64,
}

/// Body tube (thin cylinder): CG at mid-length.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BodyTube {
    pub length_m: f64,
    pub outer_diameter_m: f64,
    pub mass_kg: f64,
}

/// Trapezoidal fin set. Uniform-sheet fins: the set's CG sits at the
/// planform's area centroid (chordwise), at the fin station.
///
/// Geometry per fin: root chord along the body, tip chord parallel to it,
/// `sweep_m` = leading-edge offset of the tip from the root leading edge,
/// `span_m` = distance root to tip. `position_from_nose_m` locates the root
/// leading edge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinSet {
    pub count: u32,
    pub root_chord_m: f64,
    pub tip_chord_m: f64,
    pub span_m: f64,
    pub sweep_m: f64,
    pub position_from_nose_m: f64,
    pub mass_kg: f64,
}

impl FinSet {
    /// Chordwise area centroid of one fin, measured aft of the root leading
    /// edge. Exact for a trapezoid: with leading edge x_le(y) = sweep·y/s and
    /// chord c(y) = c_r + (c_t − c_r)·y/s, the centroid is
    /// ∫(x_le + c/2)·c dy / ∫c dy — polynomial integrals with closed form.
    pub fn planform_centroid_m(&self) -> f64 {
        let (cr, ct, m) = (self.root_chord_m, self.tip_chord_m, self.sweep_m);
        let area2 = cr + ct; // 2·area / span
        // ∫ x_le·c dy over y∈[0,1] (span factored out): m·(cr + 2·ct)/6
        let le_term = m * (cr + 2.0 * ct) / 6.0;
        // ∫ (c²/2) dy = (cr² + cr·ct + ct²)/6
        let chord_term = (cr * cr + cr * ct + ct * ct) / 6.0;
        (le_term + chord_term) / (area2 / 2.0)
    }
}

/// Anything modeled as a point: payload, motor mount hardware, chute,
/// launch lug, recovery wadding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointMass {
    pub name: String,
    pub mass_kg: f64,
    pub position_from_nose_m: f64,
}

/// Single-stage parametric vehicle. The motor is not part of the dry
/// vehicle; it is supplied per-flight so CG can track propellant burn.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vehicle {
    pub name: String,
    pub nose: NoseCone,
    pub body: BodyTube,
    pub fins: FinSet,
    #[serde(default)]
    pub point_masses: Vec<PointMass>,
    /// Forward end of the motor, aft of the nose tip.
    pub motor_position_from_nose_m: f64,
}

impl Vehicle {
    pub fn length_m(&self) -> f64 {
        self.nose.length_m + self.body.length_m
    }

    /// Reference diameter (body tube).
    pub fn diameter_m(&self) -> f64 {
        self.body.outer_diameter_m
    }

    pub fn dry_mass_kg(&self) -> f64 {
        self.nose.mass_kg
            + self.body.mass_kg
            + self.fins.mass_kg
            + self.point_masses.iter().map(|p| p.mass_kg).sum::<f64>()
    }

    /// Dry CG aft of the nose tip: mass-weighted component CGs.
    pub fn dry_cg_from_nose_m(&self) -> f64 {
        let nose_cg = 2.0 / 3.0 * self.nose.length_m;
        let body_cg = self.nose.length_m + self.body.length_m / 2.0;
        let fin_cg = self.fins.position_from_nose_m + self.fins.planform_centroid_m();
        let mut moment = self.nose.mass_kg * nose_cg
            + self.body.mass_kg * body_cg
            + self.fins.mass_kg * fin_cg;
        for p in &self.point_masses {
            moment += p.mass_kg * p.position_from_nose_m;
        }
        moment / self.dry_mass_kg()
    }

    /// Motor CG station: motor midpoint, held fixed through the burn
    /// (uniform propellant consumption along the grain — the standard
    /// small-motor assumption; casing dominates the CG anyway).
    pub fn motor_cg_from_nose_m(&self, motor: &Motor) -> f64 {
        self.motor_position_from_nose_m + motor.length_mm / 1000.0 / 2.0
    }

    pub fn loaded_mass_at(&self, motor: &Motor, t: f64) -> f64 {
        self.dry_mass_kg() + motor.mass_at(t)
    }

    /// Loaded CG at burn time `t`, aft of the nose tip.
    pub fn cg_at(&self, motor: &Motor, t: f64) -> f64 {
        let dry = self.dry_mass_kg();
        let motor_mass = motor.mass_at(t);
        (dry * self.dry_cg_from_nose_m() + motor_mass * self.motor_cg_from_nose_m(motor))
            / (dry + motor_mass)
    }
}
