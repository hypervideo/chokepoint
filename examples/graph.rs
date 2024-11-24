use chokepoint::{
    normal_distribution,
    test_sink::{
        TestPayload,
        TestSink,
    },
    ChokeSettings,
    ChokeSettingsOrder,
    ChokeSink,
    ChokeStream,
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
use tokio_stream::wrappers::ReceiverStream;

#[derive(Parser)]
struct Args {
    #[clap(short, long, action)]
    verbose: bool,

    #[clap()]
    mode: Mode,

    #[clap(short, default_value = "250")]
    n: usize,

    #[clap(short, long)]
    output: Option<PathBuf>,

    #[clap(long, value_parser = parse_ordering, default_value = "unordered")]
    ordering: ChokeSettingsOrder,

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
    #[clap(long, default_value = "0.0")]
    mean: f64,

    #[clap(long, default_value = "0.0")]
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

    // file or stdout
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
}

async fn stream(
    mut out: Box<dyn std::io::Write>,
    Args {
        n,
        ordering,
        latency_distribution: LatencyDistribution { mean, stddev },
        ..
    }: Args,
) {
    let (tx, rx) = mpsc::channel(1);

    let mut traffic_shaper = ChokeStream::<TestPayload>::new(
        Box::new(ReceiverStream::new(rx)),
        ChokeSettings::default()
            .set_ordering(Some(ordering))
            .set_latency_distribution(chokepoint::normal_distribution(mean, stddev, mean + stddev * 3.0))
            // .set_bandwidth_limit(Some(250))
            .set_corrupt_probability(Some(0.0)),
    );

    tokio::spawn(async move {
        for i in 0..n {
            tx.send(TestPayload::new(i)).await.unwrap();
        }
    });

    writeln!(out, "i,received,created,delta").unwrap();

    while let Some(packet) = traffic_shaper.next().await {
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
        latency_distribution: LatencyDistribution { mean, stddev },
        ..
    }: Args,
) {
    let mut sink = ChokeSink::new(
        TestSink::default(),
        ChokeSettings::default()
            .set_ordering(Some(ordering))
            .set_latency_distribution(normal_distribution(mean, stddev, mean + stddev * 3.0)),
    );

    for i in 0..n {
        sink.send(TestPayload::new(i)).await.unwrap();
    }

    sink.close().await.unwrap();

    writeln!(out, "i,received,created,delta").unwrap();
    let items = sink.into_inner().received.into_inner().into_iter().collect::<Vec<_>>();

    for (received, TestPayload { created, i }) in items {
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
