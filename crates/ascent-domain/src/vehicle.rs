//! Vehicle model tree (v0.3 Step 1) — the single source of truth every
//! other layer derives from: mass properties feed the sim, geometry feeds
//! Barrowman and the mesh, and the command spine (Step 2) mutates it.
//!
//! Schema contract lives in `docs/VEHICLE_TREE.md`. Conventions:
//! - Datum is the nose tip; x grows aft, meters.
//! - Structural parts (NoseCone, BodyTube, Transition) stack in root
//!   order and define the airframe length.
//! - Attachments (FinSet, MotorMount, Parachute, MassComponent) live as
//!   children of a structural part and never advance the stack.
//! - Every part carries explicit mass; the rollup is a sum, never a guess.
//! - Mass-property approximations per part type are documented in
//!   `VEHICLE_TREE.md` and mirrored by worksheet fixture tests here.

use serde::{Deserialize, Serialize};

/// Stable per-vehicle part identifier. Commands (Step 2) and mesh part
/// ranges (Step 3) address parts by this id, so ids never get reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PartId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoseShape {
    TangentOgive,
    Conical,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PartKind {
    NoseCone {
        shape: NoseShape,
        length_m: f64,
        base_radius_m: f64,
        mass_g: f64,
    },
    BodyTube {
        length_m: f64,
        outer_radius_m: f64,
        wall_mm: f64,
        mass_g: f64,
    },
    Transition {
        length_m: f64,
        fore_radius_m: f64,
        aft_radius_m: f64,
        mass_g: f64,
    },
    /// Trailing edge sits flush with the parent tube's aft end.
    FinSet {
        count: u32,
        root_chord_m: f64,
        tip_chord_m: f64,
        span_m: f64,
        sweep_m: f64,
        thickness_mm: f64,
        mass_g: f64,
    },
    MotorMount {
        motor_designation: String,
        length_m: f64,
        /// Fore end of the mount, measured from the parent's fore end.
        position_m: f64,
        mass_g: f64,
    },
    Parachute {
        diameter_cm: f64,
        cd: f64,
        /// Packed location, measured from the parent's fore end.
        position_m: f64,
        mass_g: f64,
    },
    MassComponent {
        name: String,
        /// Point location, measured from the parent's fore end.
        position_m: f64,
        mass_g: f64,
    },
}

impl PartKind {
    pub fn mass_g(&self) -> f64 {
        match self {
            PartKind::NoseCone { mass_g, .. }
            | PartKind::BodyTube { mass_g, .. }
            | PartKind::Transition { mass_g, .. }
            | PartKind::FinSet { mass_g, .. }
            | PartKind::MotorMount { mass_g, .. }
            | PartKind::Parachute { mass_g, .. }
            | PartKind::MassComponent { mass_g, .. } => *mass_g,
        }
    }

    /// Structural parts stack and define the airframe; everything else
    /// attaches to one.
    pub fn is_structural(&self) -> bool {
        matches!(
            self,
            PartKind::NoseCone { .. } | PartKind::BodyTube { .. } | PartKind::Transition { .. }
        )
    }

    fn structural_length_m(&self) -> f64 {
        match self {
            PartKind::NoseCone { length_m, .. }
            | PartKind::BodyTube { length_m, .. }
            | PartKind::Transition { length_m, .. } => *length_m,
            _ => 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Part {
    pub id: PartId,
    pub kind: PartKind,
    #[serde(default)]
    pub children: Vec<Part>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Vehicle {
    pub name: String,
    pub parts: Vec<Part>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MassProperties {
    pub total_mass_g: f64,
    pub cg_from_nose_m: f64,
    /// Pitch/yaw moment of inertia about the CG, kg·m².
    pub longitudinal_moi_kg_m2: f64,
}

impl Vehicle {
    /// Airframe length: the structural stack, nose tip to aft end.
    pub fn stack_length_m(&self) -> f64 {
        self.parts.iter().map(|p| p.kind.structural_length_m()).sum()
    }

    /// Tree invariants: root parts structural, attachments only as
    /// children of structural parts, ids unique, masses non-negative.
    pub fn validate(&self) -> Result<(), String> {
        let mut seen = std::collections::BTreeSet::new();
        for part in &self.parts {
            if !part.kind.is_structural() {
                return Err(format!(
                    "part {} is an attachment and cannot sit at root level",
                    part.id.0
                ));
            }
            validate_part(part, &mut seen)?;
        }
        Ok(())
    }

    /// Mass, CG, and pitch MOI rollup. Approximations per part type are
    /// the documented worksheet formulas (VEHICLE_TREE.md): slender parts
    /// are thin rods (m·L²/12 own inertia), attachments without length
    /// are point masses; parallel-axis everywhere.
    pub fn mass_properties(&self) -> MassProperties {
        // Pass 1: collect (mass_kg, cg_station_m, own_moi_kg_m2) per part.
        let mut items: Vec<(f64, f64, f64)> = Vec::new();
        let mut cursor = 0.0;
        for part in &self.parts {
            let fore = cursor;
            let length = part.kind.structural_length_m();
            collect_mass_items(part, fore, length, &mut items);
            cursor += length;
        }

        let total_kg: f64 = items.iter().map(|(m, _, _)| m).sum();
        if total_kg == 0.0 {
            return MassProperties {
                total_mass_g: 0.0,
                cg_from_nose_m: 0.0,
                longitudinal_moi_kg_m2: 0.0,
            };
        }
        let cg = items.iter().map(|(m, x, _)| m * x).sum::<f64>() / total_kg;
        let moi = items
            .iter()
            .map(|(m, x, own)| own + m * (x - cg) * (x - cg))
            .sum::<f64>();
        MassProperties {
            total_mass_g: total_kg * 1000.0,
            cg_from_nose_m: cg,
            longitudinal_moi_kg_m2: moi,
        }
    }
}

fn validate_part(part: &Part, seen: &mut std::collections::BTreeSet<u32>) -> Result<(), String> {
    if !seen.insert(part.id.0) {
        return Err(format!("duplicate part id {}", part.id.0));
    }
    if part.kind.mass_g() < 0.0 {
        return Err(format!("part {} has negative mass", part.id.0));
    }
    for child in &part.children {
        if child.kind.is_structural() {
            return Err(format!(
                "part {} is structural and must sit at root level, not nested",
                child.id.0
            ));
        }
        if !child.children.is_empty() {
            return Err(format!("attachment {} cannot have children", child.id.0));
        }
        validate_part(child, seen)?;
    }
    Ok(())
}

/// Worksheet mass model per part (see VEHICLE_TREE.md):
/// - NoseCone: thin conical shell — CG at 2/3·L aft of the tip (the same
///   convention ascent-aero's dry_cg_from_nose_m uses), thin-rod own MOI.
/// - BodyTube / Transition: CG at mid-length, thin-rod own MOI.
/// - FinSet: point mass, CG half a root chord ahead of the parent's aft end.
/// - MotorMount: CG at mount mid-length, thin-rod own MOI.
/// - Parachute / MassComponent: point mass at its position.
fn collect_mass_items(
    part: &Part,
    parent_fore_m: f64,
    parent_length_m: f64,
    items: &mut Vec<(f64, f64, f64)>,
) {
    let mass_kg = part.kind.mass_g() / 1000.0;
    match &part.kind {
        PartKind::NoseCone { length_m, .. } => {
            items.push((
                mass_kg,
                parent_fore_m + 2.0 / 3.0 * length_m,
                mass_kg * length_m * length_m / 12.0,
            ));
        }
        PartKind::BodyTube { length_m, .. } | PartKind::Transition { length_m, .. } => {
            items.push((
                mass_kg,
                parent_fore_m + length_m / 2.0,
                mass_kg * length_m * length_m / 12.0,
            ));
        }
        PartKind::FinSet { root_chord_m, .. } => {
            items.push((
                mass_kg,
                parent_fore_m + parent_length_m - root_chord_m / 2.0,
                0.0,
            ));
        }
        PartKind::MotorMount {
            length_m,
            position_m,
            ..
        } => {
            items.push((
                mass_kg,
                parent_fore_m + position_m + length_m / 2.0,
                mass_kg * length_m * length_m / 12.0,
            ));
        }
        PartKind::Parachute { position_m, .. } | PartKind::MassComponent { position_m, .. } => {
            items.push((mass_kg, parent_fore_m + position_m, 0.0));
        }
    }
    for child in &part.children {
        collect_mass_items(child, parent_fore_m, parent_length_m, items);
    }
}

/// The Estes Alpha III as a tree, dimensioned to match the currently
/// validated configuration: 25 mm caliber, 12-caliber stack (3-caliber
/// tangent-ogive nose + 9-caliber tube — the same proportions the mesh
/// and planar model used provisionally), 34.0 g dry mass, fin planform
/// matching the rendered fins (root 2 cal, tip 1 cal, span 1.5 cal).
/// Mass split is a documented estimate summing exactly to the validated
/// total; per-part masses refine when real teardown data arrives.
pub fn reference_vehicle() -> Vehicle {
    Vehicle {
        name: "Estes Alpha III".into(),
        parts: vec![
            Part {
                id: PartId(1),
                kind: PartKind::NoseCone {
                    shape: NoseShape::TangentOgive,
                    length_m: 0.075,
                    base_radius_m: 0.0125,
                    mass_g: 8.0,
                },
                children: vec![],
            },
            Part {
                id: PartId(2),
                kind: PartKind::BodyTube {
                    length_m: 0.225,
                    outer_radius_m: 0.0125,
                    wall_mm: 0.5,
                    mass_g: 15.0,
                },
                children: vec![
                    Part {
                        id: PartId(3),
                        kind: PartKind::FinSet {
                            count: 3,
                            root_chord_m: 0.05,
                            tip_chord_m: 0.025,
                            span_m: 0.0375,
                            sweep_m: 0.0,
                            thickness_mm: 3.0,
                            mass_g: 6.0,
                        },
                        children: vec![],
                    },
                    Part {
                        id: PartId(4),
                        kind: PartKind::Parachute {
                            diameter_cm: 30.0,
                            cd: 0.75,
                            position_m: 0.05,
                            mass_g: 3.0,
                        },
                        children: vec![],
                    },
                    Part {
                        id: PartId(5),
                        kind: PartKind::MotorMount {
                            motor_designation: "C6".into(),
                            length_m: 0.07,
                            position_m: 0.155,
                            mass_g: 2.0,
                        },
                        children: vec![],
                    },
                ],
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_mass_matches_the_validated_dry_mass_exactly() {
        let v = reference_vehicle();
        assert_eq!(v.mass_properties().total_mass_g, 34.0);
    }

    #[test]
    fn reference_stack_is_twelve_calibers() {
        let v = reference_vehicle();
        assert!((v.stack_length_m() - 12.0 * 0.025).abs() < 1e-12);
    }

    #[test]
    fn reference_validates() {
        reference_vehicle().validate().unwrap();
    }

    /// Worksheet fixture: CG recomputed independently, term by term, with
    /// the documented per-part conventions (datum = nose tip).
    #[test]
    fn reference_cg_matches_the_hand_worksheet() {
        // nose:  8 g at (2/3)·0.075                   = 0.05 m
        // tube: 15 g at 0.075 + 0.5·0.225             = 0.1875 m
        // fins:  6 g at 0.075 + 0.225 − 0.5·0.05      = 0.275 m
        // chute: 3 g at 0.075 + 0.05                  = 0.125 m
        // mount: 2 g at 0.075 + 0.155 + 0.5·0.07      = 0.265 m
        let expected = (8.0 * 0.05 + 15.0 * 0.1875 + 6.0 * 0.275 + 3.0 * 0.125 + 2.0 * 0.265)
            / 34.0;
        let props = reference_vehicle().mass_properties();
        assert!(
            (props.cg_from_nose_m - expected).abs() < 1e-12,
            "cg {} != worksheet {}",
            props.cg_from_nose_m,
            expected
        );
    }

    /// Worksheet fixture: pitch MOI about the CG, thin-rod own terms plus
    /// parallel-axis transfers, recomputed independently.
    #[test]
    fn reference_moi_matches_the_hand_worksheet() {
        let cg = reference_vehicle().mass_properties().cg_from_nose_m;
        let rod = |m_kg: f64, l: f64| m_kg * l * l / 12.0;
        let transfer = |m_kg: f64, x: f64| m_kg * (x - cg) * (x - cg);
        let expected = rod(0.008, 0.075)
            + transfer(0.008, 0.05)
            + rod(0.015, 0.225)
            + transfer(0.015, 0.1875)
            + transfer(0.006, 0.275)
            + transfer(0.003, 0.125)
            + rod(0.002, 0.07)
            + transfer(0.002, 0.265);
        let props = reference_vehicle().mass_properties();
        assert!(
            (props.longitudinal_moi_kg_m2 - expected).abs() < 1e-15,
            "moi {} != worksheet {}",
            props.longitudinal_moi_kg_m2,
            expected
        );
    }

    #[test]
    fn toml_roundtrip_is_byte_identical() {
        let v = reference_vehicle();
        let first = toml::to_string_pretty(&v).unwrap();
        let reparsed: Vehicle = toml::from_str(&first).unwrap();
        assert_eq!(reparsed, v);
        let second = toml::to_string_pretty(&reparsed).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn nested_structural_parts_are_rejected() {
        let mut v = reference_vehicle();
        v.parts[1].children.push(Part {
            id: PartId(9),
            kind: PartKind::BodyTube {
                length_m: 0.1,
                outer_radius_m: 0.0125,
                wall_mm: 0.5,
                mass_g: 5.0,
            },
            children: vec![],
        });
        assert!(v.validate().unwrap_err().contains("structural"));
    }

    #[test]
    fn attachments_cannot_sit_at_root() {
        let mut v = reference_vehicle();
        v.parts.push(Part {
            id: PartId(9),
            kind: PartKind::MassComponent {
                name: "lug".into(),
                position_m: 0.0,
                mass_g: 1.0,
            },
            children: vec![],
        });
        assert!(v.validate().unwrap_err().contains("attachment"));
    }

    #[test]
    fn duplicate_ids_are_rejected() {
        let mut v = reference_vehicle();
        v.parts[1].children[0].id = PartId(1);
        assert!(v.validate().unwrap_err().contains("duplicate"));
    }

    /// A second structural segment (multi-tube stack) rolls up correctly:
    /// the cursor advances so the added tube sits aft of the first.
    #[test]
    fn multi_segment_stack_rolls_up_with_the_cursor() {
        let mut v = reference_vehicle();
        v.parts.push(Part {
            id: PartId(10),
            kind: PartKind::BodyTube {
                length_m: 0.1,
                outer_radius_m: 0.0125,
                wall_mm: 0.5,
                mass_g: 6.0,
            },
            children: vec![],
        });
        let props = v.mass_properties();
        assert_eq!(props.total_mass_g, 40.0);
        // New tube's CG term: 6 g at 0.3 + 0.05 = 0.35 m.
        let expected_cg = (34.0 * (5.7675 / 34.0) + 6.0 * 0.35) / 40.0;
        // 5.7675/34 is the reference CG from the worksheet fixture above.
        assert!((props.cg_from_nose_m - expected_cg).abs() < 1e-9);
        assert!((v.stack_length_m() - 0.4).abs() < 1e-12);
    }
}
