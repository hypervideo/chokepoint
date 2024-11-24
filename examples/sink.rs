use chokepoint::{
    normal_distribution,
    test_sink::{
        TestPayload,
        TestSink,
    },
    ChokeSettings,
    ChokeSettingsOrder,
    ChokeSink,
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
    // run2().await;
}

async fn run() {
    let settings = ChokeSettings::default().set_latency_distribution(normal_distribution(20.0, 100.0, 1000.0));

    let mut sink = ChokeSink::new(TestSink::default(), settings);

    for i in 0..10usize {
        println!("[{i}] emitting");
        sink.send(TestPayload::new(i)).await.unwrap();
    }

    println!("closing sink");
    sink.close().await.unwrap();

    let received = sink.into_inner().received.into_inner();
    println!("received: {:?}", received);
}

#[allow(dead_code)]
async fn run2() {
    let mut sink = ChokeSink::new(
        TestSink::default(),
        ChokeSettings::default()
            .set_latency_distribution(normal_distribution(5.0, 10.0, 100.0))
            .set_ordering(Some(ChokeSettingsOrder::Backpressure)),
    );

    for i in 0..10usize {
        sink.send(TestPayload::new(i)).await.unwrap();
    }

    sink.close().await.unwrap();

    let mut received = sink
        .into_inner()
        .received
        .into_inner()
        .into_iter()
        .map(|(_, TestPayload { i, .. })| i)
        .collect::<Vec<_>>();
    received.sort();

    assert_eq!(received.len(), 10, "{:?}", received);
    assert_eq!(received, (0..10).collect::<Vec<_>>(), "{:?}", received);
}
