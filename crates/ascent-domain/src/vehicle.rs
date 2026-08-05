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
    /// Joins two stages. Everything from the coupler down (aft) is the
    /// lower stage; the coupler drops with it at separation.
    StageCoupler {
        length_m: f64,
        outer_radius_m: f64,
        mass_g: f64,
        /// Delay after the lower stage's burnout before it separates.
        separation_delay_s: f64,
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
            | PartKind::StageCoupler { mass_g, .. }
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
            PartKind::NoseCone { .. }
                | PartKind::BodyTube { .. }
                | PartKind::Transition { .. }
                | PartKind::StageCoupler { .. }
        )
    }

    fn structural_length_m(&self) -> f64 {
        match self {
            PartKind::NoseCone { length_m, .. }
            | PartKind::BodyTube { length_m, .. }
            | PartKind::Transition { length_m, .. }
            | PartKind::StageCoupler { length_m, .. } => *length_m,
            _ => 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Part {
    pub id: PartId,
    pub kind: PartKind,
    /// Measured hardware mass. When set, mass and CG rollups prefer this
    /// value while retaining the design mass in `kind` for reconciliation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub as_built_mass_g: Option<f64>,
    #[serde(default)]
    pub children: Vec<Part>,
}

impl Part {
    pub fn effective_mass_g(&self) -> f64 {
        self.as_built_mass_g.unwrap_or_else(|| self.kind.mass_g())
    }
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
        self.parts
            .iter()
            .map(|p| p.kind.structural_length_m())
            .sum()
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
        for (i, part) in self.parts.iter().enumerate() {
            if matches!(part.kind, PartKind::StageCoupler { .. })
                && (i == 0 || i == self.parts.len() - 1)
            {
                return Err(format!(
                    "stage coupler {} must sit between structural parts, not at the stack end",
                    part.id.0
                ));
            }
        }
        Ok(())
    }

    /// Stage segments, nose-first (top stage first; burn order is the
    /// reverse). A stage begins at each `StageCoupler` — the coupler
    /// drops with the stage below it. A vehicle with no couplers is one
    /// stage.
    pub fn stages(&self) -> Vec<StageInfo> {
        let mut boundaries = vec![0];
        for (i, part) in self.parts.iter().enumerate() {
            if matches!(part.kind, PartKind::StageCoupler { .. }) {
                boundaries.push(i);
            }
        }
        boundaries.push(self.parts.len());
        boundaries.dedup();

        boundaries
            .windows(2)
            .map(|w| {
                let (start, end) = (w[0], w[1]);
                let segment = &self.parts[start..end];
                let mut dry_mass_g = 0.0;
                let mut motor_designation = None;
                for part in segment {
                    dry_mass_g += subtree_mass_g(part);
                    find_motor_mount(part, &mut motor_designation);
                }
                let separation_delay_s = match &self.parts[start].kind {
                    PartKind::StageCoupler {
                        separation_delay_s, ..
                    } => Some(*separation_delay_s),
                    _ => None,
                };
                StageInfo {
                    part_range: (start, end),
                    dry_mass_g,
                    motor_designation,
                    separation_delay_s,
                }
            })
            .collect()
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

/// One stage segment of the root stack, nose-first indices.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StageInfo {
    /// Root-part indices `[start, end)` of this stage's segment.
    pub part_range: (usize, usize),
    /// Dry mass of the segment including all attachments, grams.
    pub dry_mass_g: f64,
    /// Designation of the first motor mount found in the segment.
    pub motor_designation: Option<String>,
    /// `Some` for stages that begin at a coupler (everything below the
    /// top stage): delay after this stage's burnout before it drops.
    pub separation_delay_s: Option<f64>,
}

fn subtree_mass_g(part: &Part) -> f64 {
    part.effective_mass_g() + part.children.iter().map(subtree_mass_g).sum::<f64>()
}

fn find_motor_mount(part: &Part, found: &mut Option<String>) {
    if found.is_none() {
        if let PartKind::MotorMount {
            motor_designation, ..
        } = &part.kind
        {
            *found = Some(motor_designation.clone());
        }
    }
    for child in &part.children {
        find_motor_mount(child, found);
    }
}

fn validate_part(part: &Part, seen: &mut std::collections::BTreeSet<u32>) -> Result<(), String> {
    if !seen.insert(part.id.0) {
        return Err(format!("duplicate part id {}", part.id.0));
    }
    if part.kind.mass_g() < 0.0
        || part
            .as_built_mass_g
            .is_some_and(|mass| !mass.is_finite() || mass < 0.0)
    {
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
    let mass_kg = part.effective_mass_g() / 1000.0;
    match &part.kind {
        PartKind::NoseCone { length_m, .. } => {
            items.push((
                mass_kg,
                parent_fore_m + 2.0 / 3.0 * length_m,
                mass_kg * length_m * length_m / 12.0,
            ));
        }
        PartKind::BodyTube { length_m, .. }
        | PartKind::Transition { length_m, .. }
        | PartKind::StageCoupler { length_m, .. } => {
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
                as_built_mass_g: None,
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
                as_built_mass_g: None,
                kind: PartKind::BodyTube {
                    length_m: 0.225,
                    outer_radius_m: 0.0125,
                    wall_mm: 0.5,
                    mass_g: 15.0,
                },
                children: vec![
                    Part {
                        id: PartId(3),
                        as_built_mass_g: None,
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
                        as_built_mass_g: None,
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
                        as_built_mass_g: None,
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

/// Two-stage variant of the reference vehicle: the Alpha III stack as the
/// sustainer, a coupler, and a finned D12 booster below it. Drag
/// separation at booster burnout (zero delay), the Estes gap-staging
/// pattern.
pub fn two_stage_reference_vehicle() -> Vehicle {
    let mut v = reference_vehicle();
    v.name = "Estes Alpha III · D12 booster".into();
    v.parts.push(Part {
        id: PartId(6),
        as_built_mass_g: None,
        kind: PartKind::StageCoupler {
            length_m: 0.02,
            outer_radius_m: 0.0125,
            mass_g: 4.0,
            separation_delay_s: 0.0,
        },
        children: vec![],
    });
    v.parts.push(Part {
        id: PartId(7),
        as_built_mass_g: None,
        kind: PartKind::BodyTube {
            length_m: 0.09,
            outer_radius_m: 0.0125,
            wall_mm: 0.5,
            mass_g: 12.0,
        },
        children: vec![
            Part {
                id: PartId(8),
                as_built_mass_g: None,
                kind: PartKind::FinSet {
                    count: 3,
                    root_chord_m: 0.06,
                    tip_chord_m: 0.03,
                    span_m: 0.045,
                    sweep_m: 0.0,
                    thickness_mm: 3.0,
                    mass_g: 8.0,
                },
                children: vec![],
            },
            Part {
                id: PartId(9),
                as_built_mass_g: None,
                kind: PartKind::MotorMount {
                    motor_designation: "D12".into(),
                    length_m: 0.07,
                    position_m: 0.02,
                    mass_g: 3.0,
                },
                children: vec![],
            },
        ],
    });
    v
}
