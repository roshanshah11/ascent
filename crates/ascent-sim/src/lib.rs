pub mod atmosphere;
pub mod engine;
pub mod events;
pub mod rocket;
#[cfg(feature = "bridge-rocketpy")]
pub mod rocketpy_bridge;
pub mod sim;
pub mod summary;

pub use atmosphere::AtmosphereModel;
pub use engine::{engines, NativeEngine, SimEngine};
pub use events::{Event, EventKind};
pub use rocket::{DragModel, Environment, Recovery, Rocket};
#[cfg(feature = "bridge-rocketpy")]
pub use rocketpy_bridge::RocketPyEngine;
pub use sim::{
    convergence_report, simulate_vertical, ConvergenceReport, SimConfig, SimResult,
};
pub use summary::{input_hash, SimSummary};
