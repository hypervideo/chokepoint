# Changelog

## [0.2.0] - 2024-11-19
### Added
- Support for wasm32-unknown-unknown target.
- Implement `ChokeItem` for `Result` and `Option`.
- Allow updating the `ChokeStreamSettings` dynamically.
- `ChokeSink` to wrap a `future::Sink`.
- `ChokeSettingsOrder` to enforce ordered or unordered packet delivery (in combination with delays)
- CLI to experiment with the library.

### Changed
- Rename types.
- Bug fixes.

