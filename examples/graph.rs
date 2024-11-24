use chokepoint::{
    normal_distribution,
    test_sink::{
        TestPayload,
        TestSink,
    },
    ChokeSettings,
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

    #[clap(long, action)]
    backpressure: bool,

    #[clap(flatten)]
    latency_distribution: LatencyDistribution,
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

    match args.mode {
        Mode::Stream => stream(args).await,
        Mode::Sink => sink(args).await,
    }
}

async fn stream(
    Args {
        n,
        backpressure,
        latency_distribution: LatencyDistribution { mean, stddev },
        ..
    }: Args,
) {
    let (tx, rx) = mpsc::channel(1);

    let mut traffic_shaper = ChokeStream::<TestPayload>::new(
        Box::new(ReceiverStream::new(rx)),
        ChokeSettings::default()
            .set_backpressure(Some(backpressure))
            .set_latency_distribution(chokepoint::normal_distribution(mean, stddev, mean + stddev * 3.0))
            // .set_bandwidth_limit(Some(250))
            .set_corrupt_probability(Some(0.0)),
    );

    tokio::spawn(async move {
        for i in 0..n {
            tx.send(TestPayload::new(i)).await.unwrap();
        }
    });

    println!("i,received,created,delta");

    while let Some(packet) = traffic_shaper.next().await {
        let now = Utc::now();
        let delta = now - packet.created;
        println!(
            "{},{},{},{}",
            packet.i,
            now.to_rfc3339(),
            packet.created.to_rfc3339(),
            delta.num_milliseconds()
        );
    }
}

async fn sink(
    Args {
        n,
        backpressure,
        latency_distribution: LatencyDistribution { mean, stddev },
        ..
    }: Args,
) {
    let mut sink = ChokeSink::new(
        TestSink::default(),
        ChokeSettings::default()
            .set_backpressure(Some(backpressure))
            .set_latency_distribution(normal_distribution(mean, stddev, mean + stddev * 3.0)),
    );

    for i in 0..n {
        sink.send(TestPayload::new(i)).await.unwrap();
    }

    sink.close().await.unwrap();

    println!("i,received,created,delta");
    let items = sink.into_inner().received.into_inner().into_iter().collect::<Vec<_>>();

    for (received, TestPayload { created, i }) in items {
        let delta = received - created;
        println!(
            "{i},{},{},{}",
            received.to_rfc3339(),
            created.to_rfc3339(),
            delta.num_milliseconds()
        );
    }
}
