# chokepoint

[![Crates.io](https://img.shields.io/crates/v/chokepoint)](https://crates.io/crates/chokepoint)
[![](https://docs.rs/chokepoint/badge.svg)](https://docs.rs/chokepoint)
[![License](https://img.shields.io/crates/l/chokepoint?color=informational&logo=mpl-2)](/LICENSE)

A more or less generic "traffic" shaper that can be used to modify the delivery of payloads.

Supports various simulated network conditions, such as:
- Delay (using a user provided function)
- Packet loss
- Packet reordering
- Packet corruption
- Packet duplication
- Bandwidth limiting

See [`TrafficShaper`] for more information.

License: MPL-2.0
