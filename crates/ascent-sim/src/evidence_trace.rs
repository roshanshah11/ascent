use ascent_domain::evidence::{
    Channel, ChannelKind, CoordinateFrame, FlightTrace, FrameKind, Sample, TimeBase, TimeSystem,
    TrackKind, TypedEvent, Unit, EVIDENCE_SCHEMA_VERSION,
};

use crate::{DispersionSummary, SimResult, SixDofResult};

const TIME_BASE_ID: &str = "simulation";
const FRAME_ID: &str = "launch-enu";

/// Adapt a point-mass result without changing or re-running it. Missing axes
/// are represented explicitly as zero-valued truth in the launch ENU frame.
pub fn flight_trace_from_vertical(
    result: &SimResult,
    input_hash: &str,
) -> Result<FlightTrace, String> {
    let samples = |values: fn(&crate::sim::Sample) -> Vec<f64>| {
        result
            .samples
            .iter()
            .map(|sample| Sample {
                time: sample.t,
                values: values(sample),
                valid: true,
            })
            .collect()
    };
    let trace = FlightTrace {
        schema_version: EVIDENCE_SCHEMA_VERSION,
        trace_id: "vertical-flight".into(),
        time_bases: simulation_time_base(),
        frames: launch_frame(),
        channels: vec![
            channel(
                "truth.position",
                ChannelKind::Position,
                Unit::Meter,
                samples(|sample| vec![0.0, 0.0, sample.altitude_m]),
                input_hash,
                true,
            ),
            channel(
                "truth.velocity",
                ChannelKind::Velocity,
                Unit::MeterPerSecond,
                samples(|sample| vec![0.0, 0.0, sample.velocity_ms]),
                input_hash,
                true,
            ),
            channel(
                "truth.mass",
                ChannelKind::Scalar,
                Unit::Kilogram,
                samples(|sample| vec![sample.mass_kg]),
                input_hash,
                false,
            ),
            channel(
                "truth.thrust",
                ChannelKind::Scalar,
                Unit::Custom("newton".into()),
                samples(|sample| vec![sample.thrust_n]),
                input_hash,
                false,
            ),
        ],
        events: result
            .events
            .iter()
            .enumerate()
            .map(|(index, event)| TypedEvent {
                id: format!("simulation-event-{index}"),
                event_type: format!("{:?}", event.kind).to_ascii_lowercase(),
                time_base_id: TIME_BASE_ID.into(),
                time: event.t,
                detector_id: "ascent-sim.interpolated-crossing".into(),
                detector_version: env!("CARGO_PKG_VERSION").into(),
                confidence: 1.0,
                source_sample_ranges: vec!["simulation integration history".into()],
                evidence_hashes: vec![input_hash.into()],
            })
            .collect(),
    };
    trace.validate()?;
    Ok(trace)
}

/// Project a detailed rigid-body result onto the same evidence contract used
/// by imported telemetry and point-mass simulations.
pub fn flight_trace_from_sixdof(result: &SixDofResult) -> Result<FlightTrace, String> {
    let input_hash = result.summary.input_hash.as_str();
    let samples = |values: fn(&crate::SixDofSample) -> Vec<f64>| {
        result
            .history
            .iter()
            .map(|sample| Sample {
                time: sample.time_s,
                values: values(sample),
                valid: true,
            })
            .collect()
    };
    let trace = FlightTrace {
        schema_version: EVIDENCE_SCHEMA_VERSION,
        trace_id: "sixdof-flight".into(),
        time_bases: simulation_time_base(),
        frames: launch_frame(),
        channels: vec![
            channel(
                "truth.position",
                ChannelKind::Position,
                Unit::Meter,
                samples(|sample| sample.position_m.to_vec()),
                input_hash,
                true,
            ),
            channel(
                "truth.velocity",
                ChannelKind::Velocity,
                Unit::MeterPerSecond,
                samples(|sample| sample.velocity_ms.to_vec()),
                input_hash,
                true,
            ),
            channel(
                "truth.attitude",
                ChannelKind::AttitudeQuaternionWxyz,
                Unit::Dimensionless,
                samples(|sample| sample.attitude_wxyz.to_vec()),
                input_hash,
                true,
            ),
            channel(
                "truth.body_rates",
                ChannelKind::BodyRates,
                Unit::RadianPerSecond,
                samples(|sample| sample.angular_rate_rad_s.to_vec()),
                input_hash,
                true,
            ),
        ],
        events: result
            .summary
            .events
            .iter()
            .enumerate()
            .map(|(index, event)| TypedEvent {
                id: format!("simulation-event-{index}"),
                event_type: event.kind.to_ascii_lowercase(),
                time_base_id: TIME_BASE_ID.into(),
                time: event.t_s,
                detector_id: "ascent-sixdof.interpolated-crossing".into(),
                detector_version: result.summary.sim_version.clone(),
                confidence: 1.0,
                source_sample_ranges: vec!["sixdof integration history".into()],
                evidence_hashes: vec![input_hash.into()],
            })
            .collect(),
    };
    trace.validate()?;
    Ok(trace)
}

/// Preserve a compact dispersion result as derived ensemble-member channels.
/// The member index is an explicit sequence domain, not simulated time.
pub fn flight_trace_from_dispersion(
    summary: &DispersionSummary,
    input_hash: &str,
) -> Result<FlightTrace, String> {
    if summary.samples as usize != summary.runs.len() {
        return Err(format!(
            "dispersion declares {} samples but contains {} runs",
            summary.samples,
            summary.runs.len()
        ));
    }
    let member_samples = |value: fn(&crate::CompactRun) -> f64| {
        summary
            .runs
            .iter()
            .enumerate()
            .map(|(index, run)| Sample {
                time: index as f64,
                values: vec![value(run)],
                valid: true,
            })
            .collect()
    };
    let derived = |id: &str, unit: Unit, samples: Vec<Sample>| Channel {
        id: id.into(),
        kind: ChannelKind::Scalar,
        track: TrackKind::Derived,
        time_base_id: "ensemble-member".into(),
        frame_id: None,
        unit,
        samples,
        uncertainty: None,
        parent_hashes: vec![input_hash.into()],
    };
    let trace = FlightTrace {
        schema_version: EVIDENCE_SCHEMA_VERSION,
        trace_id: format!("dispersion-seed-{}", summary.seed),
        time_bases: vec![TimeBase {
            id: "ensemble-member".into(),
            system: TimeSystem::JournalSequence,
            epoch: Some("zero-based deterministic member index".into()),
        }],
        frames: vec![],
        channels: vec![
            derived(
                "ensemble.apogee",
                Unit::Meter,
                member_samples(|run| run.apogee_m),
            ),
            derived(
                "ensemble.landing_range",
                Unit::Meter,
                member_samples(|run| run.landing_range_m),
            ),
            derived(
                "ensemble.max_aoa",
                Unit::Radian,
                member_samples(|run| run.max_aoa_deg.to_radians()),
            ),
        ],
        events: vec![],
    };
    trace.validate()?;
    Ok(trace)
}

fn channel(
    id: &str,
    kind: ChannelKind,
    unit: Unit,
    samples: Vec<Sample>,
    input_hash: &str,
    framed: bool,
) -> Channel {
    Channel {
        id: id.into(),
        kind,
        track: TrackKind::Simulated,
        time_base_id: TIME_BASE_ID.into(),
        frame_id: framed.then(|| FRAME_ID.into()),
        unit,
        samples,
        uncertainty: None,
        parent_hashes: vec![input_hash.into()],
    }
}

fn simulation_time_base() -> Vec<TimeBase> {
    vec![TimeBase {
        id: TIME_BASE_ID.into(),
        system: TimeSystem::SimulationElapsed,
        epoch: None,
    }]
}

fn launch_frame() -> Vec<CoordinateFrame> {
    vec![CoordinateFrame {
        id: FRAME_ID.into(),
        kind: FrameKind::EastNorthUp,
        parent_id: None,
    }]
}
