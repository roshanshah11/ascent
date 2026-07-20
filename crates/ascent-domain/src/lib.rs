//! Domain foundations every other crate builds on: motors and the vehicle
//! model tree. Zero renderer, zero UI, zero engine imports. The curated
//! surface is:
//!
//! - `motor`: [`Motor`] (thrust/mass curves, RASP provenance) and
//!   [`MotorError`]
//! - `eng_import`: [`parse_eng`] for RASP `.eng` files
//! - `registry`: [`MotorRegistry`] (bundled + imported motors) and
//!   [`ImportOutcome`]
//! - `vehicle`: the model tree ([`Vehicle`], [`Part`], [`PartId`],
//!   [`PartKind`], [`NoseShape`]), its rollups ([`MassProperties`],
//!   [`Vehicle::stages`] → [`StageInfo`]) and the reference fixtures
//!   ([`reference_vehicle`], [`two_stage_reference_vehicle`])

pub mod eng_import;
pub mod evidence;
pub mod motor;
pub mod reference;
pub mod registry;
pub mod telemetry;
pub mod vehicle;

pub use eng_import::{parse_eng, EngImportError};
pub use motor::{Motor, MotorError};
pub use registry::{ImportOutcome, MotorRegistry};
pub use vehicle::{
    reference_vehicle, two_stage_reference_vehicle, MassProperties, NoseShape, Part, PartId,
    PartKind, StageInfo, Vehicle,
};
