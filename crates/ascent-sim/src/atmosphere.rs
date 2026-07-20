use serde::{Deserialize, Serialize};

/// Air density model.
///
/// `Standard` is the 1976 US Standard Atmosphere troposphere layer
/// (valid 0–11 km, which covers everything below IREC 45k ft plus margin).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AtmosphereModel {
    /// Fixed density everywhere — used by analytic fixtures.
    ConstantDensity(f64),
    /// 1976 US Standard Atmosphere, troposphere layer.
    Standard,
    /// Measured densities from an imported profile (v0.5), linearly
    /// interpolated between levels and clamped outside them. Points must
    /// be sorted by strictly increasing altitude — the importing side
    /// (`AtmosphereProfile::validate`) guarantees it.
    Layered { points: Vec<DensityPoint> },
}

/// One measured level of a [`AtmosphereModel::Layered`] atmosphere.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DensityPoint {
    pub altitude_m: f64,
    pub density_kg_m3: f64,
}

const T0_K: f64 = 288.15;
const P0_PA: f64 = 101_325.0;
const LAPSE_K_PER_M: f64 = 0.0065;
const R_SPECIFIC: f64 = 287.053;
const G0: f64 = 9.80665;

impl AtmosphereModel {
    /// Density in kg/m³ at geometric altitude `h` metres above sea level.
    pub fn density_at(&self, h: f64) -> f64 {
        match self {
            AtmosphereModel::ConstantDensity(rho) => *rho,
            AtmosphereModel::Standard => {
                let h = h.clamp(0.0, 11_000.0);
                let t = T0_K - LAPSE_K_PER_M * h;
                let p = P0_PA * (t / T0_K).powf(G0 / (R_SPECIFIC * LAPSE_K_PER_M));
                p / (R_SPECIFIC * t)
            }
            AtmosphereModel::Layered { points } => match points.as_slice() {
                [] => 0.0,
                [only] => only.density_kg_m3,
                points => {
                    let first = &points[0];
                    let last = &points[points.len() - 1];
                    if h <= first.altitude_m {
                        return first.density_kg_m3;
                    }
                    if h >= last.altitude_m {
                        return last.density_kg_m3;
                    }
                    let above = points.partition_point(|p| p.altitude_m <= h);
                    let (lo, hi) = (&points[above - 1], &points[above]);
                    let frac = (h - lo.altitude_m) / (hi.altitude_m - lo.altitude_m);
                    lo.density_kg_m3 + frac * (hi.density_kg_m3 - lo.density_kg_m3)
                }
            },
        }
    }
}
