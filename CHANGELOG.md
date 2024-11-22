# Changelog

## [0.2.0] - 2024-11-19
### Added
- Support for wasm32-unknown-unknown target.
- Implement `ChokeItem` for `Result` and `Option`.
- Allow updating the `ChokeStreamSettings` dynamically.
- `ChokeSink` to wrap a `future::Sink`.
- optional backpressure to wait with consuming the next item until the previous one has been processed.

### Changed
- Rename types.
- Bug fixes.
