//! Barrowman aerodynamics and stability over the tree-derived vehicle
//! view. `pub` here is a promise; the curated surface is:
//!
//! - `vehicle`: [`Vehicle`] (+ [`Vehicle::from_tree`]) with its component
//!   structs [`NoseCone`], [`BodyTube`], [`FinSet`], [`PointMass`]
//! - `barrowman`: per-component CNα/CP ([`nose_cn`], [`fin_set_cn`],
//!   [`ComponentCn`]) and the CNα-weighted [`total_cp_from_nose_m`]
//! - `stability`: margin over the burn ([`stability_calibers_at`],
//!   [`stability_pct_of_length_at`], [`stability_envelope`])
//! - `constraints`: cited rule packs ([`RulePack`], [`Rule`],
//!   [`CheckResult`], [`FlightQuantities`], [`Comparator`])

pub mod barrowman;
pub mod constraints;
pub mod stability;
pub mod vehicle;

pub use barrowman::{fin_set_cn, nose_cn, total_cp_from_nose_m, ComponentCn, NoseShape};
pub use constraints::{CheckResult, Comparator, FlightQuantities, Rule, RulePack};
pub use stability::{stability_calibers_at, stability_envelope, stability_pct_of_length_at};
pub use vehicle::{BodyTube, FinSet, NoseCone, PointMass, Vehicle};
