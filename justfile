default:
    just --list

example-stream:
    cargo run --example stream

example-sink:
    cargo run --example sink

examples: example-stream example-sink

test: && examples
    cargo nextest run
    cargo test --doc

graph:
    cargo run -q --example graph | tee >(graph - -x 'now' -y 'delta')
