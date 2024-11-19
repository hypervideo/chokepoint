//! A generic stream transformer that can be used to "shape traffic", e.g. to simulate network conditions.
//!
//! [![Crates.io](https://img.shields.io/crates/v/chokepoint)](https://crates.io/crates/chokepoint)
//! [![](https://docs.rs/chokepoint/badge.svg)](https://docs.rs/chokepoint)
//! [![License](https://img.shields.io/crates/l/chokepoint?color=informational&logo=mpl-2)](/LICENSE)
//!
//! Supports various simulated network conditions, such as:
//! - Delay (using a user provided function)
//! - Packet loss
//! - Packet reordering
//! - Packet corruption
//! - Packet duplication
//! - Bandwidth limiting
//!
//! See [`TrafficShaper`] for more information.

mod latency;
mod payload;
mod traffic_shaper;

pub use latency::*;
pub use payload::TrafficShaperPayload;
pub use traffic_shaper::TrafficShaper;
