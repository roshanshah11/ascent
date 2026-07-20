use serde::{Deserialize, Serialize};

/// A solid rocket motor backed by a sampled thrust curve.
///
/// Follows the RASP convention: the curve has an implicit (0 s, 0 N) starting
/// point, and the last sample must have zero thrust (burnout).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Motor {
    pub designation: String,
    pub manufacturer: String,
    /// Exact RASP header source line for imported motors, excluding its newline.
    /// Bundled JSON motors omit this field to preserve their serialized form.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_header: Option<String>,
    pub total_mass_kg: f64,
    pub propellant_mass_kg: f64,
    pub expected_total_impulse_ns: f64,
    /// (time s, thrust N) samples, strictly increasing in time.
    pub thrust_curve: Vec<(f64, f64)>,
    #[serde(default)]
    pub provenance: serde_json::Value,
    #[serde(default)]
    pub diameter_mm: f64,
    #[serde(default)]
    pub length_mm: f64,
    #[serde(default)]
    pub expected_burn_time_s: f64,
    #[serde(default)]
    pub expected_avg_thrust_n: f64,
    #[serde(default)]
    pub expected_max_thrust_n: f64,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum MotorError {
    #[error("thrust curve is empty")]
    EmptyCurve,
    #[error("thrust curve time samples must be strictly increasing (index {0})")]
    NonMonotonicTime(usize),
    #[error("thrust curve first sample must have time > 0 (implicit 0,0 start)")]
    NonPositiveStartTime,
    #[error("thrust sample at index {0} is negative")]
    NegativeThrust(usize),
    #[error("thrust curve must end at zero thrust, got {0} N")]
    NonZeroFinalThrust(f64),
    #[error("propellant mass {prop} kg exceeds total mass {total} kg")]
    PropellantExceedsTotal { prop: f64, total: f64 },
    #[error("mass must be positive")]
    NonPositiveMass,
    #[error(
        "computed total impulse {computed:.3} N·s differs from expected {expected:.3} N·s by more than {tolerance:.0}%"
    )]
    ImpulseMismatch {
        computed: f64,
        expected: f64,
        tolerance: f64,
    },
}

impl Motor {
    /// Load a motor from its JSON representation and validate it.
    pub fn from_json(json: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let motor: Motor = serde_json::from_str(json)?;
        motor.validate()?;
        Ok(motor)
    }

    /// Validate curve shape, masses, and that the integrated impulse matches
    /// the declared expected impulse within 5%.
    pub fn validate(&self) -> Result<(), MotorError> {
        if self.thrust_curve.is_empty() {
            return Err(MotorError::EmptyCurve);
        }
        if self.thrust_curve[0].0 <= 0.0 {
            return Err(MotorError::NonPositiveStartTime);
        }
        for (i, &(t, thrust)) in self.thrust_curve.iter().enumerate() {
            if i > 0 && t <= self.thrust_curve[i - 1].0 {
                return Err(MotorError::NonMonotonicTime(i));
            }
            if thrust < 0.0 {
                return Err(MotorError::NegativeThrust(i));
            }
            let _ = t;
        }
        let final_thrust = self.thrust_curve.last().unwrap().1;
        if final_thrust != 0.0 {
            return Err(MotorError::NonZeroFinalThrust(final_thrust));
        }
        if self.total_mass_kg <= 0.0 || self.propellant_mass_kg <= 0.0 {
            return Err(MotorError::NonPositiveMass);
        }
        if self.propellant_mass_kg > self.total_mass_kg {
            return Err(MotorError::PropellantExceedsTotal {
                prop: self.propellant_mass_kg,
                total: self.total_mass_kg,
            });
        }
        const IMPULSE_TOLERANCE: f64 = 0.05;
        let computed = self.total_impulse();
        if self.expected_total_impulse_ns > 0.0 {
            let rel =
                (computed - self.expected_total_impulse_ns).abs() / self.expected_total_impulse_ns;
            if rel > IMPULSE_TOLERANCE {
                return Err(MotorError::ImpulseMismatch {
                    computed,
                    expected: self.expected_total_impulse_ns,
                    tolerance: IMPULSE_TOLERANCE * 100.0,
                });
            }
        }
        Ok(())
    }

    /// Time of the last thrust sample (burnout), seconds.
    pub fn burn_time(&self) -> f64 {
        self.thrust_curve.last().map(|&(t, _)| t).unwrap_or(0.0)
    }

    /// Linearly interpolated thrust at time `t` seconds.
    ///
    /// Implicit (0,0) start point; zero before t=0 and after burnout.
    pub fn thrust_at(&self, t: f64) -> f64 {
        if t <= 0.0 || t >= self.burn_time() {
            return 0.0;
        }
        let mut prev = (0.0, 0.0);
        for &(st, sf) in &self.thrust_curve {
            if t <= st {
                let span = st - prev.0;
                if span <= 0.0 {
                    return sf;
                }
                let frac = (t - prev.0) / span;
                return prev.1 + frac * (sf - prev.1);
            }
            prev = (st, sf);
        }
        0.0
    }

    /// Total impulse in N·s by trapezoidal integration, including the
    /// implicit (0,0) start point.
    pub fn total_impulse(&self) -> f64 {
        self.cumulative_impulse_at(self.burn_time())
    }

    /// Impulse delivered from ignition through time `t`, N·s.
    fn cumulative_impulse_at(&self, t: f64) -> f64 {
        if t <= 0.0 {
            return 0.0;
        }
        let mut acc = 0.0;
        let mut prev = (0.0_f64, 0.0_f64);
        for &(st, sf) in &self.thrust_curve {
            if t >= st {
                acc += (st - prev.0) * (prev.1 + sf) / 2.0;
                prev = (st, sf);
            } else {
                let f_t = self.thrust_at(t);
                acc += (t - prev.0) * (prev.1 + f_t) / 2.0;
                return acc;
            }
        }
        acc
    }

    /// Propellant mass consumed by time `t`, in kg.
    ///
    /// Mass depletion is proportional to the fraction of total impulse
    /// delivered by time `t` (standard RASP/OpenRocket assumption).
    pub fn propellant_consumed_at(&self, t: f64) -> f64 {
        let total = self.total_impulse();
        if total <= 0.0 {
            return 0.0;
        }
        let frac = (self.cumulative_impulse_at(t) / total).clamp(0.0, 1.0);
        frac * self.propellant_mass_kg
    }

    /// Total motor mass remaining at time `t`, in kg (casing + unburned propellant).
    pub fn mass_at(&self, t: f64) -> f64 {
        self.total_mass_kg - self.propellant_consumed_at(t)
    }
}
