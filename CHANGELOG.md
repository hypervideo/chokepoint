# Changelog

## Unreleased

### Added

- Add option `only_drop_when_bandwidth_limit_reached` to bandwidth limit settings. When enabled, drop packets even if the `bandwidth_limit` has not been reached when processing packets.

## [0.4.2] - 2024-12-01

### Changed

- Display debug logs only when verbose flag is passed during packet duplication.

## [0.4.1] - 2024-11-28

### Changed

- Changed how the bandwidth drop rate is implemented for more realistic behavior.

## [0.4.0] - 2024-11-25

### Added

- Allow setting bandwidth limits with drop probability.

## [0.3.0] - 2024-11-24

- `ChokeSettingsOrder` to enforce ordered or unordered packet delivery (in combination with delays)
- CLI to experiment with the library.

## [0.2.0] - 2024-11-19

### Added

- Support for wasm32-unknown-unknown target.
- Implement `ChokeItem` for `Result` and `Option`.
- Allow updating the `ChokeStreamSettings` dynamically.
- `ChokeSink` to wrap a `future::Sink`.

### Changed

- Rename types.
- Bug fixes.

