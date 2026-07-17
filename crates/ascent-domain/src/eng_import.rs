//! Parser for the RASP `.eng` thrust-curve interchange format.
//!
//! See `docs/RASP_FORMAT.md` for the format spec this parser implements.

use serde_json::Value;
use thiserror::Error;

use crate::motor::Motor;

/// A parse failure, tagged with the 1-indexed source line that caused it.
#[derive(Debug, Error, PartialEq)]
pub enum EngImportError {
    #[error("line {line}: {message}")]
    Malformed { line: usize, message: String },
}

fn malformed(line: usize, message: impl Into<String>) -> EngImportError {
    EngImportError::Malformed {
        line,
        message: message.into(),
    }
}

/// Parse RASP `.eng` text into one or more motors.
///
/// A file may contain multiple motor entries separated by comment lines.
/// `provenance` is attached verbatim (cloned) to every motor parsed from
/// this source, per the caller's request.
pub fn parse_eng(text: &str, provenance: &Value) -> Result<Vec<Motor>, EngImportError> {
    let lines: Vec<&str> = text.lines().collect();
    let mut motors = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let line_no = i + 1;
        let trimmed = lines[i].trim();
        if trimmed.is_empty() || trimmed.starts_with(';') {
            i += 1;
            continue;
        }

        // Header line: common_name diameter_mm length_mm delays propellant_kg
        // loaded_mass_kg manufacturer.
        let fields: Vec<&str> = trimmed.split_whitespace().collect();
        if fields.len() != 7 {
            return Err(malformed(
                line_no,
                format!("expected 7 header fields, got {}", fields.len()),
            ));
        }
        let designation = fields[0].to_string();
        let diameter_mm: f64 = fields[1]
            .parse()
            .map_err(|_| malformed(line_no, format!("invalid diameter_mm '{}'", fields[1])))?;
        let length_mm: f64 = fields[2]
            .parse()
            .map_err(|_| malformed(line_no, format!("invalid length_mm '{}'", fields[2])))?;
        // fields[3] is the delay list (e.g. "0-3-5-7"); not modeled on Motor.
        let propellant_mass_kg: f64 = fields[4]
            .parse()
            .map_err(|_| malformed(line_no, format!("invalid propellant_kg '{}'", fields[4])))?;
        let total_mass_kg: f64 = fields[5]
            .parse()
            .map_err(|_| malformed(line_no, format!("invalid loaded_mass_kg '{}'", fields[5])))?;
        let manufacturer = fields[6].to_string();

        i += 1;

        let mut thrust_curve: Vec<(f64, f64)> = Vec::new();
        let mut last_data_line = line_no;
        let mut terminated_at_zero = false;

        while i < lines.len() {
            let data_line_no = i + 1;
            let data_trimmed = lines[i].trim();
            if data_trimmed.is_empty() || data_trimmed.starts_with(';') {
                break;
            }

            let data_fields: Vec<&str> = data_trimmed.split_whitespace().collect();
            if data_fields.len() != 2 {
                return Err(malformed(
                    data_line_no,
                    format!(
                        "expected 2 data fields (time_s thrust_n), got {}",
                        data_fields.len()
                    ),
                ));
            }
            let t: f64 = data_fields[0].parse().map_err(|_| {
                malformed(data_line_no, format!("non-numeric time_s '{}'", data_fields[0]))
            })?;
            let thrust: f64 = data_fields[1].parse().map_err(|_| {
                malformed(data_line_no, format!("non-numeric thrust_n '{}'", data_fields[1]))
            })?;

            thrust_curve.push((t, thrust));
            last_data_line = data_line_no;
            i += 1;

            if thrust == 0.0 {
                // RASP: an early or final zero terminates the entry.
                terminated_at_zero = true;
                break;
            }
        }

        if !terminated_at_zero {
            let last_thrust = thrust_curve.last().map(|&(_, f)| f).unwrap_or(0.0);
            return Err(malformed(
                last_data_line,
                format!(
                    "entry ended without a terminal zero-thrust sample (last thrust {last_thrust} N); \
                     truncated or missing terminal zero"
                ),
            ));
        }

        let expected_burn_time_s = thrust_curve.last().map(|&(t, _)| t).unwrap_or(0.0);
        let expected_total_impulse_ns = trapezoidal_impulse(&thrust_curve);
        let expected_max_thrust_n = thrust_curve
            .iter()
            .map(|&(_, f)| f)
            .fold(0.0_f64, f64::max);
        let expected_avg_thrust_n = if expected_burn_time_s > 0.0 {
            expected_total_impulse_ns / expected_burn_time_s
        } else {
            0.0
        };

        motors.push(Motor {
            designation,
            manufacturer,
            total_mass_kg,
            propellant_mass_kg,
            expected_total_impulse_ns,
            thrust_curve,
            provenance: provenance.clone(),
            diameter_mm,
            length_mm,
            expected_burn_time_s,
            expected_avg_thrust_n,
            expected_max_thrust_n,
        });
    }

    if motors.is_empty() {
        return Err(malformed(
            lines.len().max(1),
            "no motor entries found in .eng source",
        ));
    }

    Ok(motors)
}

/// Trapezoidal integration including the implicit (0, 0) RASP start point.
/// Mirrors `Motor::total_impulse`'s method so a freshly parsed motor's
/// `expected_total_impulse_ns` always matches its own computed impulse.
fn trapezoidal_impulse(curve: &[(f64, f64)]) -> f64 {
    let mut acc = 0.0;
    let mut prev = (0.0_f64, 0.0_f64);
    for &(t, f) in curve {
        acc += (t - prev.0) * (prev.1 + f) / 2.0;
        prev = (t, f);
    }
    acc
}
