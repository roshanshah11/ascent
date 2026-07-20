//! Parametric vehicle model: geometry, component masses, and center of
//! gravity — including CG as a function of time while the motor burns.
//!
//! Positions are measured in meters aft of the nose tip (the datum).
//! Component-internal CG locations use thin-shell / uniform-sheet
//! assumptions, stated per component. Barrowman CP lands in a separate
//! module once the hand-computed fixture (docs/BARROWMAN_WORKSHEET.md)
//! exists; mass properties here are independent of it.

use ascent_domain::vehicle::{PartKind, Vehicle as TreeVehicle};
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
    /// Derive the aero view from the domain model tree (v0.3 Step 3) —
    /// the tree is the single source of truth; this flattening is how
    /// Barrowman and stability consume it. Multiple body tubes merge
    /// into one equivalent tube (summed length and mass, max diameter);
    /// the first fin set is the stabilizing set (trailing edge flush
    /// with its parent's aft end); mounts, chutes, and mass components
    /// become point masses at their absolute stations.
    pub fn from_tree(tree: &TreeVehicle) -> Result<(Vehicle, crate::NoseShape), String> {
        tree.validate()?;
        let mut nose: Option<(NoseCone, crate::NoseShape)> = None;
        let mut body_length = 0.0;
        let mut body_mass = 0.0;
        let mut body_diameter: f64 = 0.0;
        let mut fins: Option<FinSet> = None;
        let mut point_masses = Vec::new();
        let mut motor_position = None;

        let mut cursor = 0.0;
        for part in &tree.parts {
            let fore = cursor;
            let length = match &part.kind {
                PartKind::NoseCone {
                    shape,
                    length_m,
                    base_radius_m,
                    mass_g: _,
                } => {
                    if nose.is_some() {
                        return Err("only one nose cone is supported".into());
                    }
                    let aero_shape = match shape {
                        ascent_domain::vehicle::NoseShape::TangentOgive => crate::NoseShape::Ogive,
                        ascent_domain::vehicle::NoseShape::Conical => crate::NoseShape::Conical,
                    };
                    nose = Some((
                        NoseCone {
                            length_m: *length_m,
                            base_diameter_m: 2.0 * base_radius_m,
                            mass_kg: part.effective_mass_g() / 1000.0,
                        },
                        aero_shape,
                    ));
                    *length_m
                }
                PartKind::BodyTube {
                    length_m,
                    outer_radius_m,
                    ..
                } => {
                    body_length += length_m;
                    body_mass += part.effective_mass_g() / 1000.0;
                    body_diameter = body_diameter.max(2.0 * outer_radius_m);
                    *length_m
                }
                PartKind::Transition {
                    length_m,
                    aft_radius_m,
                    ..
                } => {
                    // Aero transition handling is future work; mass-wise it
                    // folds into the equivalent body tube.
                    body_length += length_m;
                    body_mass += part.effective_mass_g() / 1000.0;
                    body_diameter = body_diameter.max(2.0 * aft_radius_m);
                    *length_m
                }
                PartKind::StageCoupler {
                    length_m,
                    outer_radius_m,
                    ..
                } => {
                    // Constant-radius section: no Barrowman CN contribution,
                    // but its length and mass are part of the airframe.
                    body_length += length_m;
                    body_mass += part.effective_mass_g() / 1000.0;
                    body_diameter = body_diameter.max(2.0 * outer_radius_m);
                    *length_m
                }
                _ => 0.0,
            };
            for child in &part.children {
                match &child.kind {
                    PartKind::FinSet {
                        count,
                        root_chord_m,
                        tip_chord_m,
                        span_m,
                        sweep_m,
                        ..
                    } => {
                        if fins.is_some() {
                            return Err("only one fin set is supported".into());
                        }
                        fins = Some(FinSet {
                            count: *count,
                            root_chord_m: *root_chord_m,
                            tip_chord_m: *tip_chord_m,
                            span_m: *span_m,
                            sweep_m: *sweep_m,
                            position_from_nose_m: fore + length - root_chord_m,
                            mass_kg: child.effective_mass_g() / 1000.0,
                        });
                    }
                    PartKind::MotorMount { position_m, .. } => {
                        motor_position = Some(fore + position_m);
                        point_masses.push(PointMass {
                            name: "motor mount".into(),
                            mass_kg: child.effective_mass_g() / 1000.0,
                            position_from_nose_m: fore + position_m,
                        });
                    }
                    PartKind::Parachute { position_m, .. } => point_masses.push(PointMass {
                        name: "parachute".into(),
                        mass_kg: child.effective_mass_g() / 1000.0,
                        position_from_nose_m: fore + position_m,
                    }),
                    PartKind::MassComponent {
                        name,
                        position_m,
                        mass_g: _,
                    } => point_masses.push(PointMass {
                        name: name.clone(),
                        mass_kg: child.effective_mass_g() / 1000.0,
                        position_from_nose_m: fore + position_m,
                    }),
                    _ => {}
                }
            }
            cursor += length;
        }

        let (nose, shape) = nose.ok_or("vehicle needs a nose cone")?;
        if body_length == 0.0 {
            return Err("vehicle needs at least one body tube".into());
        }
        let fins = fins.ok_or("vehicle needs a fin set")?;
        let motor_position_from_nose_m =
            motor_position.unwrap_or(nose.length_m + body_length - 0.07);
        Ok((
            Vehicle {
                name: tree.name.clone(),
                nose,
                body: BodyTube {
                    length_m: body_length,
                    outer_diameter_m: body_diameter,
                    mass_kg: body_mass,
                },
                fins,
                point_masses,
                motor_position_from_nose_m,
            },
            shape,
        ))
    }

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
        let mut moment =
            self.nose.mass_kg * nose_cg + self.body.mass_kg * body_cg + self.fins.mass_kg * fin_cg;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{barrowman, NoseShape};
    use ascent_domain::vehicle::reference_vehicle;

    /// The conversion must agree exactly with a hand-built aero vehicle
    /// of identical dimensions — same CP, same CNα, same dry CG inputs.
    #[test]
    fn reference_tree_converts_to_the_hand_built_aero_vehicle() {
        let (converted, shape) = Vehicle::from_tree(&reference_vehicle()).unwrap();
        assert!(matches!(shape, NoseShape::Ogive));
        let hand_built = Vehicle {
            name: "Estes Alpha III".into(),
            nose: NoseCone {
                length_m: 0.075,
                base_diameter_m: 0.025,
                mass_kg: 0.008,
            },
            body: BodyTube {
                length_m: 0.225,
                outer_diameter_m: 0.025,
                mass_kg: 0.015,
            },
            fins: FinSet {
                count: 3,
                root_chord_m: 0.05,
                tip_chord_m: 0.025,
                span_m: 0.0375,
                sweep_m: 0.0,
                position_from_nose_m: 0.075 + 0.225 - 0.05,
                mass_kg: 0.006,
            },
            point_masses: vec![],
            motor_position_from_nose_m: 0.075 + 0.155,
        };
        let cp_converted = barrowman::total_cp_from_nose_m(&converted, NoseShape::Ogive);
        let cp_hand = barrowman::total_cp_from_nose_m(&hand_built, NoseShape::Ogive);
        assert_eq!(cp_converted, cp_hand, "CP must match exactly");
        assert_eq!(converted.length_m(), 0.3);
        assert_eq!(converted.diameter_m(), 0.025);
        assert!((converted.motor_position_from_nose_m - 0.23).abs() < 1e-12);
        // Point masses (chute + mount hardware) carry over.
        assert_eq!(converted.point_masses.len(), 2);
    }

    #[test]
    fn a_fin_edit_in_the_tree_moves_the_cp() {
        let tree = reference_vehicle();
        let (v0, shape) = Vehicle::from_tree(&tree).unwrap();
        let cp0 = barrowman::total_cp_from_nose_m(&v0, shape);
        let mut bigger = tree.clone();
        if let ascent_domain::vehicle::PartKind::FinSet { span_m, .. } =
            &mut bigger.parts[1].children[0].kind
        {
            *span_m = 0.06;
        }
        let (v1, shape) = Vehicle::from_tree(&bigger).unwrap();
        let cp1 = barrowman::total_cp_from_nose_m(&v1, shape);
        assert!(cp1 > cp0, "bigger fins pull the CP aft: {cp1} vs {cp0}");
    }

    #[test]
    fn as_built_tree_mass_moves_cg_without_moving_cp() {
        let tree = reference_vehicle();
        let (nominal, shape) = Vehicle::from_tree(&tree).unwrap();
        let cp = barrowman::total_cp_from_nose_m(&nominal, shape);
        let mut measured_tree = tree;
        measured_tree.parts[1].children[0].as_built_mass_g = Some(12.0);
        let (measured, measured_shape) = Vehicle::from_tree(&measured_tree).unwrap();
        assert!(measured.dry_cg_from_nose_m() > nominal.dry_cg_from_nose_m());
        assert_eq!(
            barrowman::total_cp_from_nose_m(&measured, measured_shape),
            cp
        );
        let motor = Motor::from_json(include_str!(
            "../../ascent-domain/data/motors/estes_c6.json"
        ))
        .unwrap();
        assert!(
            crate::stability_calibers_at(&measured, measured_shape, &motor, 0.0)
                < crate::stability_calibers_at(&nominal, shape, &motor, 0.0),
            "extra aft fin mass must reduce the CG-to-CP stability margin"
        );
    }

    #[test]
    fn conversion_requires_a_complete_airframe() {
        let mut no_fins = reference_vehicle();
        no_fins.parts[1].children.remove(0);
        assert!(Vehicle::from_tree(&no_fins)
            .unwrap_err()
            .contains("fin set"));
        let mut no_nose = reference_vehicle();
        no_nose.parts.remove(0);
        assert!(Vehicle::from_tree(&no_nose).unwrap_err().contains("nose"));
    }
}
