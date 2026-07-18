pub mod eng_import;
pub mod motor;
pub mod registry;
pub mod vehicle;

pub use eng_import::{parse_eng, EngImportError};
pub use motor::{Motor, MotorError};
pub use registry::{ImportOutcome, MotorRegistry};
pub use vehicle::{
    reference_vehicle, MassProperties, NoseShape, Part, PartId, PartKind, Vehicle,
};
