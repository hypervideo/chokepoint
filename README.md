# chokepoint

[![Crates.io](https://img.shields.io/crates/v/chokepoint)](https://crates.io/crates/chokepoint)
[![](https://docs.rs/chokepoint/badge.svg)](https://docs.rs/chokepoint)
[![License](https://img.shields.io/crates/l/chokepoint?color=informational&logo=mpl-2)](/LICENSE)

A library for simulating "traffic shaping" in Rust based on a generic `futures::Stream` and `futures::Sink`
transformer that can be used to modify the delivery of items. The main purpose is to simulate various network
conditions such as:
- Delay (using a user provided function)
- Packet loss
- Packet reordering
- Packet corruption
- Packet duplication
- Bandwidth limiting

See [`TrafficShaper`] for more information and an example.

### chokepoint command line tool

At [./cli](./cli) you can find a simple cli tool for interactive exploration. Using a tool like [graph-cli](https://github.com/mcastorina/graph-cli/) you can visualize the output. Here is an example to showcase delay, jitter and bandwidth:

![Example](./docs/demo.png)

```sh
$ chokepoint --help
Usage: chokepoint [OPTIONS] <MODE>

Arguments:
  <MODE>  Simulate a sink or a stream [possible values: stream, sink]

Options:
  -v, --verbose

  -n <N>
          Number of packets to send [default: 250]
  -o, --output <OUTPUT>
          Output file (csv) with packet timing information
  -r, --packet-rate <PACKET_RATE>
          Send rate in packets per second
  -s, --packet-size <PACKET_SIZE>
          Packet size in bytes [default: 1B]
      --ordering <ORDERING>
          [default: ordered]
  -l, --bandwidth-limit <BANDWIDTH_LIMIT>
          Bandwidth limit
      --mean <MEAN>
          Mean latency in ms [default: 0.0]
      --stddev <STDDEV>
          Standard deviation of latency in ms (aka jitter) [default: 0.0]
  -h, --help
          Print help
```
