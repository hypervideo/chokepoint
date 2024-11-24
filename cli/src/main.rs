use chokepoint::{
    normal_distribution,
    ChokeSettings,
    ChokeSettingsOrder,
    ChokeSink,
    ChokeStream,
};
use chokepoint_test_helpers::{
    TestPayload,
    TestSink,
};
use chrono::prelude::*;
use clap::{
    Parser,
    ValueEnum,
};
use futures::{
    stream::StreamExt,
    SinkExt,
};
use std::path::PathBuf;
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;

#[macro_use]
extern crate tracing;

#[derive(Parser)]
struct Args {
    #[clap(short, long, action)]
    verbose: bool,

    #[clap(help = "Simulate a sink or a stream")]
    mode: Mode,

    #[clap(short, default_value = "250", help = "Number of packets to send")]
    n: usize,

    #[clap(short, long, help = "Output file (csv) with packet timing information")]
    output: Option<PathBuf>,

    #[clap(short = 'r', long, help = "Send rate in packets per second")]
    packet_rate: Option<usize>,

    #[clap(short = 's', long, help = "Packet size in bytes", default_value = "1B")]
    packet_size: bytesize::ByteSize,

    #[clap(long, value_parser = parse_ordering, default_value = "ordered")]
    ordering: ChokeSettingsOrder,

    #[clap(short = 'l', long, help = "Bandwidth limit")]
    bandwidth_limit: Option<bytesize::ByteSize>,

    #[clap(flatten)]
    latency_distribution: LatencyDistribution,
}

fn parse_ordering(s: &str) -> Result<ChokeSettingsOrder, &'static str> {
    match s {
        "unordered" => Ok(ChokeSettingsOrder::Unordered),
        "ordered" => Ok(ChokeSettingsOrder::Ordered),
        "backpressure" => Ok(ChokeSettingsOrder::Backpressure),
        _ => Err("invalid ordering"),
    }
}

#[derive(Debug, clap::Args)]
#[group(required = false, multiple = true)]
struct LatencyDistribution {
    #[clap(long, default_value = "0.0", help = "Mean latency in ms")]
    mean: f64,

    #[clap(
        long,
        default_value = "0.0",
        help = "Standard deviation of latency in ms (aka jitter)"
    )]
    stddev: f64,
}

#[derive(ValueEnum, Debug, Clone, Copy)]
enum Mode {
    Stream,
    Sink,
}

#[tokio::main]
async fn main() {
    let args: Args = Args::parse();

    if args.verbose {
        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::builder().parse_lossy("trace"))
            .with_span_events(
                tracing_subscriber::fmt::format::FmtSpan::NEW | tracing_subscriber::fmt::format::FmtSpan::CLOSE,
            )
            .init();
    }

    let now = Utc::now();
    let n = args.n;

    let out = match &args.output {
        Some(path) => {
            let file = std::fs::File::create(path).unwrap();
            Box::new(std::io::BufWriter::new(file)) as Box<dyn std::io::Write>
        }
        None => Box::new(std::io::stdout()) as Box<dyn std::io::Write>,
    };

    match args.mode {
        Mode::Stream => stream(out, args).await,
        Mode::Sink => sink(out, args).await,
    }

    let elapsed = (Utc::now() - now).num_milliseconds();
    let ms_per_packet = elapsed as f64 / n as f64;
    info!("done in {}ms ms/packet={:.2}", elapsed, ms_per_packet);
}

async fn stream(
    mut out: Box<dyn std::io::Write>,
    Args {
        n,
        ordering,
        packet_rate,
        packet_size,
        latency_distribution: LatencyDistribution { mean, stddev },
        bandwidth_limit,
        ..
    }: Args,
) {
    let (tx, rx) = mpsc::unbounded_channel();

    let mut stream = ChokeStream::<TestPayload>::new(
        Box::new(UnboundedReceiverStream::new(rx)),
        ChokeSettings::default()
            .set_ordering(Some(ordering))
            .set_latency_distribution(chokepoint::normal_distribution(mean, stddev, mean + stddev * 3.0))
            .set_bandwidth_limit(bandwidth_limit.map(|b| b.as_u64() as usize))
            .set_corrupt_probability(Some(0.0)),
    );

    tokio::spawn(async move {
        let packet_size = packet_size.as_u64() as usize;
        let delay = packet_rate.map(|packet_rate| std::time::Duration::from_micros(1_000_000 / packet_rate as u64));
        debug!("using delay={:?}", delay);
        let now = Utc::now();

        const CHUNKED: bool = false;

        if CHUNKED {
            let chunk_size = 10;
            for chunk in (0..n).collect::<Vec<_>>().chunks(chunk_size) {
                for i in chunk {
                    tx.send(TestPayload::new(*i, packet_size)).unwrap();
                }
                if let Some(delay) = delay {
                    tokio::time::sleep(delay * chunk_size as u32).await;
                }
            }
        } else {
            for i in 0..n {
                tx.send(TestPayload::new(i, packet_size)).unwrap();
                if let Some(delay) = delay {
                    tokio::time::sleep(delay).await;
                }
            }
        }

        debug!(
            "sent {} packets in {}µs ({}µs/packet)",
            n,
            (Utc::now() - now).num_microseconds().unwrap(),
            (Utc::now() - now).num_microseconds().unwrap() / n as i64
        );
    });

    writeln!(out, "i,received,created,delta").unwrap();

    while let Some(packet) = stream.next().await {
        let now = Utc::now();
        let delta = now - packet.created;
        writeln!(
            out,
            "{},{},{},{}",
            packet.i,
            now.to_rfc3339(),
            packet.created.to_rfc3339(),
            delta.num_milliseconds()
        )
        .unwrap();
    }
}

async fn sink(
    mut out: Box<dyn std::io::Write>,
    Args {
        n,
        ordering,
        packet_rate,
        packet_size,
        latency_distribution: LatencyDistribution { mean, stddev },
        bandwidth_limit,
        ..
    }: Args,
) {
    let mut sink = ChokeSink::new(
        TestSink::default(),
        ChokeSettings::default()
            .set_ordering(Some(ordering))
            .set_bandwidth_limit(bandwidth_limit.map(|b| b.as_u64() as usize))
            .set_latency_distribution(normal_distribution(mean, stddev, mean + stddev * 3.0))
            .set_corrupt_probability(Some(0.0)),
    );

    {
        let packet_size = packet_size.as_u64() as usize;
        let delay = packet_rate.map(|packet_rate| std::time::Duration::from_micros(1_000_000 / packet_rate as u64));
        debug!("using delay={:?}", delay);
        let now = Utc::now();

        const CHUNKED: bool = false;

        if CHUNKED {
            let chunk_size = 10;
            for chunk in (0..n).collect::<Vec<_>>().chunks(chunk_size) {
                for i in chunk {
                    sink.send(TestPayload::new(*i, packet_size)).await.unwrap();
                }
                if let Some(delay) = delay {
                    tokio::time::sleep(delay * chunk_size as u32).await;
                }
            }
        } else {
            for i in 0..n {
                sink.send(TestPayload::new(i, packet_size)).await.unwrap();
                if let Some(delay) = delay {
                    tokio::time::sleep(delay).await;
                }
            }
        }

        debug!(
            "sent {} packets in {}µs ({}µs/packet)",
            n,
            (Utc::now() - now).num_microseconds().unwrap(),
            (Utc::now() - now).num_microseconds().unwrap() / n as i64
        );
    }

    sink.close().await.unwrap();

    writeln!(out, "i,received,created,delta").unwrap();
    let items = sink.into_inner().received.into_inner().into_iter().collect::<Vec<_>>();

    for (received, TestPayload { created, i, .. }) in items {
        let delta = received - created;
        writeln!(
            out,
            "{i},{},{},{}",
            received.to_rfc3339(),
            created.to_rfc3339(),
            delta.num_milliseconds()
        )
        .unwrap();
    }
}
