//! [![Crates.io](https://img.shields.io/crates/v/chokepoint)](https://crates.io/crates/chokepoint)
//! [![](https://docs.rs/chokepoint/badge.svg)](https://docs.rs/chokepoint)
//! [![License](https://img.shields.io/crates/l/chokepoint?color=informational&logo=mpl-2)](/LICENSE)
//!
//! A generic `futures::Stream` and `futures::Sink` transformer that can be used to "shape traffic", e.g. to simulate
//! network conditions.
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

#[macro_use]
extern crate tracing;

#[macro_use]
extern crate pin_project;

mod item;
mod latency;
mod settings;
mod sink;
mod stream;
pub(crate) mod time;

#[cfg(any(test, debug_assertions))]
pub mod test_sink;

pub use item::ChokeItem;
pub use latency::*;
pub use settings::ChokeSettings;
pub use sink::ChokeSink;
pub use stream::ChokeStream;
