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

# just graph stream -n 500 --mean 25 --stddev 5 --ordering ordered --packet-size 1KB --bandwidth-limit 190KB --bandwidth-drop-prob 0.59 --packet-rate 241
graph *args="":
    chokepoint -o example.csv {{ args }}
    graph example.csv -x 'received' --xlabel "packet" -y 'delta' --ylabel "time in ms"
    rm example.csv
