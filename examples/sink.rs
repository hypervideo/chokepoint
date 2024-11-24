use chokepoint::{
    normal_distribution,
    ChokeSettings,
    ChokeSink,
};
use chokepoint_test_helpers::{
    TestPayload,
    TestSink,
};
use futures::SinkExt as _;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::builder().parse_lossy("trace"))
        .with_span_events(
            tracing_subscriber::fmt::format::FmtSpan::NEW | tracing_subscriber::fmt::format::FmtSpan::CLOSE,
        )
        .init();

    run().await;
}

async fn run() {
    let settings = ChokeSettings::default().set_latency_distribution(normal_distribution(20.0, 100.0, 1000.0));

    let mut sink = ChokeSink::new(TestSink::default(), settings);

    for i in 0..10usize {
        println!("[{i}] emitting");
        sink.send(TestPayload::new(i, 1)).await.unwrap();
    }

    println!("closing sink");
    sink.close().await.unwrap();

    let received = sink.into_inner().received.into_inner();
    println!("received: {:?}", received);
}
