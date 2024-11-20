use bytes::Bytes;
use chokepoint::{
    normal_distribution,
    ChokeStream,
    ChokeStreamSettings,
};
use chrono::{
    prelude::*,
    Duration,
};
use futures::stream::StreamExt;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

#[tokio::main]
async fn main() {
    let (tx, rx) = mpsc::channel(5);

    let mut settings = ChokeStreamSettings::default();
    let settings_tx = settings.settings_updater();

    let mut traffic_shaper = ChokeStream::new(Box::new(ReceiverStream::new(rx)), settings);

    // You can send new settings to the TrafficShaper at any time (normally you would do this on creation, this is just
    // to showcase that).
    settings_tx
        .send(
            ChokeStreamSettings::default()
                .set_latency_distribution(normal_distribution(10.0, 15.0, 100.0))
                .set_drop_probability(0.1)
                .set_corrupt_probability(0.0)
                .set_bandwidth_limit(100),
        )
        .await
        .unwrap();

    // Spawn a task to send packets into the TrafficShaper
    tokio::spawn(async move {
        for i in 0..10usize {
            let mut data = Vec::new();
            let now = Utc::now().timestamp_nanos_opt().unwrap();
            data.extend_from_slice(&now.to_le_bytes());
            data.extend_from_slice(&i.to_le_bytes());
            println!("[{i}] emitting packet");
            tx.send(Bytes::from(data)).await.unwrap();
        }
    });

    while let Some(packet) = traffic_shaper.next().await {
        let now = Utc::now().timestamp_nanos_opt().unwrap();
        let then = Duration::nanoseconds(i64::from_le_bytes(packet[0..8].try_into().unwrap()));
        let i = usize::from_le_bytes(packet[8..16].try_into().unwrap());
        let delta = Duration::nanoseconds(now - then.num_nanoseconds().unwrap());
        println!("[{i}] {}ms", delta.num_milliseconds());
    }
}
