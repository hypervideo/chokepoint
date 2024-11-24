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

graph *args="":
    # cargo run -q --example graph -- {{ args }} | tee >(graph - -x 'i' -y 'delta')
    cargo run --release -q --example graph -- -o example.csv {{ args }}
    graph example.csv -x 'i' -y 'delta'
    # graph example.csv -x 'received' -y 'delta'
    # graph example.csv -x 'created' -y 'delta'
    rm example.csv
    # cargo run -q --example graph | tee >(graph - -x 'created' -y 'received')
    # cargo run -q --example graph | tee >(graph - -x 'i' -y 'created')
    # cargo run -q --example graph | tee >(graph - -x 'created' -y 'i')
