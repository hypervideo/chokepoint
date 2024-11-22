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
use futures::{
    stream::StreamExt,
    SinkExt,
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

#[tokio::main]
async fn main() {
    // tracing_subscriber::fmt()
    //     .with_env_filter(tracing_subscriber::EnvFilter::builder().parse_lossy("trace"))
    //     .with_span_events(
    //         tracing_subscriber::fmt::format::FmtSpan::NEW | tracing_subscriber::fmt::format::FmtSpan::CLOSE,
    //     )
    //     .init();

    stream().await;
    // sink().await;
}

#[allow(dead_code)]
async fn stream() {
    let (tx, rx) = mpsc::channel(1);

    let mut traffic_shaper = ChokeStream::new(
        Box::new(ReceiverStream::new(rx)),
        ChokeSettings::default()
            .set_backpressure(Some(true))
            .set_latency_distribution(chokepoint::normal_distribution(5.0, 1.0, 100.0))
            // .set_bandwidth_limit(Some(250))
            .set_corrupt_probability(Some(0.0)),
    );

    tokio::spawn(async move {
        for i in 0..50usize {
            tx.send(TestPayload::new(i)).await.unwrap();
        }
    });

    println!("i,now,then,delta");

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

#[allow(dead_code)]
async fn sink() {
    let mut sink = ChokeSink::new(
        TestSink::default(),
        ChokeSettings::default()
            .set_backpressure(Some(true))
            // .set_backpressure(Some(false))
            .set_latency_distribution(normal_distribution(50.0, 1.0, 100.0)),
    );

    for i in 0..20usize {
        sink.send(TestPayload::new(i)).await.unwrap();
    }

    sink.close().await.unwrap();

    println!("i,now,then,delta");
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
