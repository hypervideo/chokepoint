default:
    just --list

example-stream:
    cargo run --release --example stream

example-sink:
    cargo run --release --example sink

examples: example-stream example-sink

test: && examples
    cargo nextest run
    cargo test --doc
