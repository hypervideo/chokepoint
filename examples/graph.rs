use bytes::Bytes;
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
    // stream().await;
    sink().await;
}

struct Payload {
    created: DateTime<Utc>,
    i: usize,
}

impl Payload {
    fn new(i: usize) -> Self {
        Self { created: Utc::now(), i }
    }

    fn serialize(&self) -> Bytes {
        let mut data = Vec::new();
        data.extend_from_slice(&self.created.timestamp_nanos_opt().unwrap().to_le_bytes());
        data.extend_from_slice(&self.i.to_le_bytes());
        Bytes::from(data)
    }

    fn deserialize(data: Bytes) -> Self {
        let created = DateTime::<Utc>::from_timestamp_nanos(i64::from_le_bytes(data[0..8].try_into().unwrap()));
        let i = usize::from_le_bytes(data[8..16].try_into().unwrap());
        Self { created, i }
    }
}

#[allow(dead_code)]
async fn stream() {
    let (tx, rx) = mpsc::channel(1);

    let mut traffic_shaper = ChokeStream::new(
        Box::new(ReceiverStream::new(rx)),
        ChokeSettings::default()
            .set_backpressure(Some(false))
            .set_latency_distribution(chokepoint::normal_distribution(25.0, 15.0, 100.0))
            .set_bandwidth_limit(Some(250))
            .set_corrupt_probability(Some(0.0)),
    );

    tokio::spawn(async move {
        for i in 0..50usize {
            tx.send(Payload::new(i).serialize()).await.unwrap();
        }
    });

    println!("i,now,then,delta");

    while let Some(packet) = traffic_shaper.next().await {
        let Payload { created, i } = Payload::deserialize(packet);
        let now = Utc::now();
        let delta = now - created;
        println!(
            "{i},{},{},{}",
            now.to_rfc3339(),
            created.to_rfc3339(),
            delta.num_milliseconds()
        );
    }
}

#[allow(dead_code)]
async fn sink() {
    // tracing_subscriber::fmt()
    //     .with_env_filter(tracing_subscriber::EnvFilter::builder().parse_lossy("trace"))
    //     .with_span_events(
    //         tracing_subscriber::fmt::format::FmtSpan::NEW | tracing_subscriber::fmt::format::FmtSpan::CLOSE,
    //     )
    //     .init();

    let mut sink = ChokeSink::new(
        TestSink::default(),
        ChokeSettings::default()
            // .set_backpressure(Some(true))
            .set_backpressure(Some(false))
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