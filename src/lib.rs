//! A more or less generic "traffic" shaper that can be used to modify the delivery of payloads.
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
