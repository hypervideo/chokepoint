default:
    just --list

example:
    cargo run --release --example simple

test:
    cargo nextest run
    cargo test --doc
