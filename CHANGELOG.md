# Changelog

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

