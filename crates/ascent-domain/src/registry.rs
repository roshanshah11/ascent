//! In-memory catalog of motors: the bundled stock set plus anything
//! imported at runtime from RASP `.eng` sources.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::eng_import::parse_eng;
use crate::motor::Motor;

const C6_JSON: &str = include_str!("../data/motors/estes_c6.json");
const B6_JSON: &str = include_str!("../data/motors/estes_b6.json");
const D12_JSON: &str = include_str!("../data/motors/estes_d12.json");

/// Motors bundled with ascent-domain (provenance-preserved JSON).
const BUNDLED_SOURCES: &[&str] = &[C6_JSON, B6_JSON, D12_JSON];

/// A lookup table of motors by designation. Starts from the bundled stock
/// set and can grow via `register_eng`. Registering a designation that
/// already exists replaces the prior entry.
#[derive(Debug, Default, Clone)]
pub struct MotorRegistry {
    motors: BTreeMap<String, Motor>,
}

/// What happened during an `.eng` import: which motor leads the file, and
/// which existing designations (bundled or previously imported) it replaced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportOutcome {
    pub first_designation: String,
    /// Designations that already existed and were overwritten by this import.
    pub replaced: Vec<String>,
}

impl MotorRegistry {
    /// Empty registry with no motors.
    pub fn new() -> Self {
        Self::default()
    }

    /// A registry preloaded with the bundled C6/B6/D12 motors.
    ///
    /// Panics if a bundled JSON source fails to parse or validate: these
    /// are checked-in fixtures, not user input, and a broken bundle is a
    /// build-time bug, not a runtime error to surface to users.
    pub fn bundled() -> Self {
        let mut registry = Self::new();
        for source in BUNDLED_SOURCES {
            let motor =
                Motor::from_json(source).expect("bundled motor JSON must parse and validate");
            registry.motors.insert(motor.designation.clone(), motor);
        }
        registry
    }

    /// Parse a RASP `.eng` source and register every motor entry it
    /// contains, attaching `provenance` to each. A designation already
    /// present in the registry is replaced, not duplicated; the outcome
    /// names any replaced designations so callers can warn the user.
    pub fn register_eng(
        &mut self,
        source: &str,
        provenance: Value,
    ) -> Result<ImportOutcome, String> {
        let motors = parse_eng(source, &provenance).map_err(|e| e.to_string())?;
        for motor in &motors {
            motor.validate().map_err(|e| e.to_string())?;
        }
        let first_designation = motors[0].designation.clone();
        let mut replaced = Vec::new();
        for motor in motors {
            let designation = motor.designation.clone();
            if self.motors.insert(designation.clone(), motor).is_some() {
                replaced.push(designation);
            }
        }
        Ok(ImportOutcome {
            first_designation,
            replaced,
        })
    }

    /// Look up a motor by designation.
    pub fn get(&self, designation: &str) -> Option<&Motor> {
        self.motors.get(designation)
    }

    /// All registered motors, in designation order.
    pub fn list(&self) -> Vec<&Motor> {
        self.motors.values().collect()
    }
}
