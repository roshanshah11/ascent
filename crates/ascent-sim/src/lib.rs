//! Flight engines: RK4 1-DOF vertical, planar 3-DOF, 6-DOF, dispersion,
//! and staged variants — all deterministic (same inputs → same bytes).
//! Engines take plain numbers; tree→parameter derivation lives upstream
//! (ascent-aero / ascent-app). The curated surface is:
//!
//! - `sim`: [`simulate_vertical`], [`SimConfig`], [`SimResult`],
//!   [`convergence_report`] / [`ConvergenceReport`]
//! - `planar`: [`simulate_planar`], [`simulate_planar_staged`] with
//!   [`PlanarVehicle`], [`PlanarStage`], [`PlanarStagedResult`],
//!   [`WindProfile`] / [`WindLayer`], [`planar_convergence`]
//! - `sixdof`: [`SixDofEngine`], [`simulate_sixdof_staged`] with
//!   [`SixDofVehicle`], [`SixDofStage`], [`SixDofLaunch`],
//!   [`Wind3DProfile`] / [`Wind3DLayer`], [`SixDofResult`]
//! - `dispersion`: [`run_dispersion`] / [`run_dispersion_observed`],
//!   [`Dispersion`], [`Variation`], [`DispersionSummary`]
//! - `rocket` / `atmosphere`: [`Rocket`], [`Environment`], [`DragModel`],
//!   [`Recovery`], [`AtmosphereModel`] / [`DensityPoint`]
//! - `profile`: [`AtmosphereProfile`] / [`ProfileLayer`] — imported
//!   wind/density soundings (v0.5), hashed into study inputs upstream
//! - `events` / `summary`: [`Event`], [`EventKind`], [`SimSummary`],
//!   [`content_hash`] / [`input_hash`]
//! - `engine`: the [`SimEngine`] trait and [`engines`] registry

pub mod atmosphere;
pub mod dispersion;
pub mod engine;
pub mod events;
pub mod evidence_trace;
pub mod planar;
pub mod profile;
pub mod reference_trace;
pub mod rocket;
#[cfg(feature = "bridge-rocketpy")]
pub mod rocketpy_bridge;
pub mod run_session;
pub mod sim;
pub mod sixdof;
pub mod summary;

pub use atmosphere::{AtmosphereModel, DensityPoint};
pub use dispersion::{
    percentile, run_dispersion, run_dispersion_observed, CompactRun, Dispersion, DispersionSummary,
    LandingEllipse, Variation, VaryParam,
};
pub use engine::{engines, NativeEngine, SimEngine};
pub use events::{Event, EventKind};
pub use evidence_trace::{
    flight_trace_from_dispersion, flight_trace_from_sixdof, flight_trace_from_vertical,
};
pub use planar::{
    planar_convergence, simulate_planar, simulate_planar_staged, PlanarConvergence, PlanarStage,
    PlanarStagedResult, PlanarSummary, PlanarVehicle, WindLayer, WindProfile,
};
pub use profile::{AtmosphereProfile, ProfileLayer};
pub use rocket::{DragModel, Drogue, Environment, Recovery, Rocket};
#[cfg(feature = "bridge-rocketpy")]
pub use rocketpy_bridge::RocketPyEngine;
pub use run_session::{
    AdvanceOutcome, FlightEvent, MissionProvenance, MissionSnapshot, RunSession, SessionState,
    SourceRef, SNAPSHOT_SCHEMA_VERSION,
};
pub use sim::{convergence_report, simulate_vertical, ConvergenceReport, SimConfig, SimResult};
pub use sixdof::{
    simulate_sixdof_staged, FlightPhase, SixDofEngine, SixDofLaunch, SixDofResult, SixDofSample,
    SixDofStage, SixDofVehicle, Wind3DLayer, Wind3DProfile,
};
pub use summary::{content_hash, input_hash, SimSummary};
