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
//!   [`Recovery`], [`AtmosphereModel`]
//! - `events` / `summary`: [`Event`], [`EventKind`], [`SimSummary`],
//!   [`content_hash`] / [`input_hash`]
//! - `engine`: the [`SimEngine`] trait and [`engines`] registry

pub mod atmosphere;
pub mod engine;
pub mod dispersion;
pub mod events;
pub mod planar;
pub mod rocket;
#[cfg(feature = "bridge-rocketpy")]
pub mod rocketpy_bridge;
pub mod sim;
pub mod sixdof;
pub mod summary;

pub use atmosphere::AtmosphereModel;
pub use dispersion::{
    percentile, run_dispersion, run_dispersion_observed, CompactRun, Dispersion, DispersionSummary, LandingEllipse,
    Variation, VaryParam,
};
pub use engine::{engines, NativeEngine, SimEngine};
pub use planar::{
    planar_convergence, simulate_planar, simulate_planar_staged, PlanarConvergence, PlanarStage,
    PlanarStagedResult, PlanarSummary, PlanarVehicle, WindLayer, WindProfile,
};
pub use events::{Event, EventKind};
pub use rocket::{DragModel, Environment, Recovery, Rocket};
#[cfg(feature = "bridge-rocketpy")]
pub use rocketpy_bridge::RocketPyEngine;
pub use sim::{
    convergence_report, simulate_vertical, ConvergenceReport, SimConfig, SimResult,
};
pub use sixdof::{
    simulate_sixdof_staged, FlightPhase, SixDofEngine, SixDofLaunch, SixDofResult, SixDofSample,
    SixDofStage, SixDofVehicle, Wind3DLayer, Wind3DProfile,
};
pub use summary::{content_hash, input_hash, SimSummary};
