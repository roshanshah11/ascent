//! Imported atmosphere profiles (v0.5): wind and optionally density vs
//! altitude, parsed from a data file the user hands us — radiosonde
//! soundings, GFS extracts, ensemble members — never fetched at runtime.
//! The profile is stored inside the document (and therefore the journal),
//! so replay carries the exact atmosphere bytes with it: no file
//! dependency, no network, byte-identical replay.
//!
//! File format: CSV with a header line. Required columns, in order:
//! `altitude_m,wind_speed_ms,wind_direction_deg` with an optional fourth
//! `density_kg_m3`. Blank lines and `#` comments are skipped. Altitudes
//! must be strictly increasing; density is all-or-none across rows.

use serde::{Deserialize, Serialize};

use crate::atmosphere::{AtmosphereModel, DensityPoint};
use crate::planar::{WindLayer, WindProfile};

/// One sounding level: wind (speed toward `direction_deg`, projected onto
/// the flight plane by the planar solver) and optionally measured density.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileLayer {
    pub altitude_m: f64,
    pub wind_speed_ms: f64,
    pub wind_direction_deg: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub density_kg_m3: Option<f64>,
}

/// A named, imported atmosphere: what the study ran under. Serialized
/// into the document and hashed into every study's input hash, so an
/// atmosphere swap provably stales results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AtmosphereProfile {
    pub name: String,
    pub layers: Vec<ProfileLayer>,
}

impl AtmosphereProfile {
    /// Parse the CSV text of a profile file. Errors name the line and
    /// what was expected — this is user data, not trusted input.
    pub fn from_csv(name: &str, text: &str) -> Result<Self, String> {
        if name.trim().is_empty() {
            return Err("profile name must not be empty".into());
        }
        let mut rows = text
            .lines()
            .enumerate()
            .map(|(i, l)| (i + 1, l.trim()))
            .filter(|(_, l)| !l.is_empty() && !l.starts_with('#'));

        let (_, header) = rows.next().ok_or("empty profile file")?;
        let cols: Vec<&str> = header.split(',').map(str::trim).collect();
        let with_density = match cols.as_slice() {
            ["altitude_m", "wind_speed_ms", "wind_direction_deg"] => false,
            ["altitude_m", "wind_speed_ms", "wind_direction_deg", "density_kg_m3"] => true,
            _ => {
                return Err(format!(
                    "bad header '{header}': expected altitude_m,wind_speed_ms,wind_direction_deg[,density_kg_m3]"
                ))
            }
        };

        let mut layers = Vec::new();
        for (line_no, row) in rows {
            let fields: Vec<&str> = row.split(',').map(str::trim).collect();
            let expected = if with_density { 4 } else { 3 };
            if fields.len() != expected {
                return Err(format!(
                    "line {line_no}: expected {expected} fields, got {}",
                    fields.len()
                ));
            }
            let num = |what: &str, s: &str| -> Result<f64, String> {
                s.parse::<f64>()
                    .ok()
                    .filter(|v| v.is_finite())
                    .ok_or_else(|| format!("line {line_no}: bad {what} '{s}'"))
            };
            layers.push(ProfileLayer {
                altitude_m: num("altitude_m", fields[0])?,
                wind_speed_ms: num("wind_speed_ms", fields[1])?,
                wind_direction_deg: num("wind_direction_deg", fields[2])?,
                density_kg_m3: if with_density {
                    Some(num("density_kg_m3", fields[3])?)
                } else {
                    None
                },
            });
        }

        let profile = AtmosphereProfile {
            name: name.trim().to_string(),
            layers,
        };
        profile.validate()?;
        Ok(profile)
    }

    /// Structural checks shared by the CSV path and JSON-carried profiles
    /// (the `set-atmosphere` command deserializes straight from the
    /// journal, which must not be able to smuggle in a bad profile).
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("profile name must not be empty".into());
        }
        if self.layers.is_empty() {
            return Err("profile has no data rows".into());
        }
        for pair in self.layers.windows(2) {
            if pair[1].altitude_m <= pair[0].altitude_m {
                return Err(format!(
                    "altitudes must be strictly increasing ({} then {})",
                    pair[0].altitude_m, pair[1].altitude_m
                ));
            }
        }
        let with_density = self.layers[0].density_kg_m3.is_some();
        for (i, layer) in self.layers.iter().enumerate() {
            if !layer.altitude_m.is_finite()
                || !layer.wind_speed_ms.is_finite()
                || !layer.wind_direction_deg.is_finite()
            {
                return Err(format!("row {}: non-finite value", i + 1));
            }
            if layer.wind_speed_ms < 0.0 {
                return Err(format!("row {}: wind speed must be >= 0", i + 1));
            }
            match layer.density_kg_m3 {
                Some(rho) if !(rho.is_finite() && rho > 0.0) => {
                    return Err(format!("row {}: density must be > 0", i + 1));
                }
                Some(_) if !with_density => {
                    return Err("density must be present on all rows or none".into());
                }
                None if with_density => {
                    return Err("density must be present on all rows or none".into());
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// The wind view the engines consume (piecewise-constant by layer).
    pub fn wind_profile(&self) -> WindProfile {
        WindProfile {
            layers: self
                .layers
                .iter()
                .map(|l| WindLayer {
                    altitude_m: l.altitude_m,
                    speed_ms: l.wind_speed_ms,
                    direction_deg: l.wind_direction_deg,
                })
                .collect(),
        }
    }

    /// The density view, when the file carried densities: a linearly
    /// interpolated layered model. `None` = keep the document's existing
    /// analytic atmosphere.
    pub fn atmosphere_model(&self) -> Option<AtmosphereModel> {
        let points: Option<Vec<DensityPoint>> = self
            .layers
            .iter()
            .map(|l| {
                l.density_kg_m3.map(|density_kg_m3| DensityPoint {
                    altitude_m: l.altitude_m,
                    density_kg_m3,
                })
            })
            .collect();
        points.map(|points| AtmosphereModel::Layered { points })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOUNDING: &str = "\
# KOUN 2026-06-01 12Z, trimmed
altitude_m,wind_speed_ms,wind_direction_deg,density_kg_m3
0,3.0,0,1.225
1000,5.5,15,1.112
3000,9.0,40,0.909
";

    #[test]
    fn parses_a_sounding_with_density() {
        let p = AtmosphereProfile::from_csv("koun-12z", SOUNDING).unwrap();
        assert_eq!(p.name, "koun-12z");
        assert_eq!(p.layers.len(), 3);
        assert_eq!(p.layers[1].wind_speed_ms, 5.5);
        assert_eq!(p.layers[2].density_kg_m3, Some(0.909));
    }

    #[test]
    fn parses_wind_only_profiles() {
        let text = "altitude_m,wind_speed_ms,wind_direction_deg\n0,2.0,0\n500,4.0,180\n";
        let p = AtmosphereProfile::from_csv("gusty", text).unwrap();
        assert_eq!(p.layers[0].density_kg_m3, None);
        assert!(p.atmosphere_model().is_none());
        let wind = p.wind_profile();
        assert_eq!(wind.layers.len(), 2);
        // 180° blows toward −x in the flight plane.
        assert!(wind.wind_x_at(600.0) < 0.0);
    }

    #[test]
    fn rejects_bad_input_with_line_numbers() {
        let bad_header = AtmosphereProfile::from_csv("x", "alt,speed\n0,1\n");
        assert!(bad_header.unwrap_err().contains("bad header"));

        let bad_value = AtmosphereProfile::from_csv(
            "x",
            "altitude_m,wind_speed_ms,wind_direction_deg\n0,fast,0\n",
        );
        assert!(bad_value.unwrap_err().contains("line 2"));

        let not_increasing = AtmosphereProfile::from_csv(
            "x",
            "altitude_m,wind_speed_ms,wind_direction_deg\n1000,1,0\n500,1,0\n",
        );
        assert!(not_increasing.unwrap_err().contains("strictly increasing"));

        let empty =
            AtmosphereProfile::from_csv("x", "altitude_m,wind_speed_ms,wind_direction_deg\n");
        assert!(empty.unwrap_err().contains("no data rows"));
    }

    #[test]
    fn density_view_interpolates_between_levels() {
        let p = AtmosphereProfile::from_csv("koun-12z", SOUNDING).unwrap();
        let model = p.atmosphere_model().unwrap();
        // Midpoint of the 0–1000 m span: mean of the bracketing densities.
        let mid = model.density_at(500.0);
        assert!((mid - (1.225 + 1.112) / 2.0).abs() < 1e-12, "mid {mid}");
        // Clamped outside the sounding's span.
        assert_eq!(model.density_at(-10.0), 1.225);
        assert_eq!(model.density_at(9000.0), 0.909);
    }

    #[test]
    fn json_round_trips_for_the_journal() {
        let p = AtmosphereProfile::from_csv("koun-12z", SOUNDING).unwrap();
        let json = serde_json::to_string(&p).unwrap();
        let back: AtmosphereProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
    }
}
