default:
    just --list

run:
    cargo run

test:
    cargo nextest run
    cargo test --doc
