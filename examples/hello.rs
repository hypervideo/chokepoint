use bytes::Bytes;
use chokepoint::TrafficShaper;
use chrono::{prelude::*, Duration};
use futures::stream::StreamExt;
use rand_distr::{num_traits::Float as _, Distribution as _, Normal};
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;

#[tokio::main]
async fn main() {
    let (tx, rx) = mpsc::unbounded_channel();

    let mut traffic_shaper = TrafficShaper::new(Box::new(UnboundedReceiverStream::new(rx)));

    // Set the latency distribution
    traffic_shaper.set_latency_distribution(|| {
        let normal = Normal::new(10.0, 15.0).unwrap(); // mean = 10ms, std dev = 15ms
        let latency = normal.sample(&mut rand::thread_rng()).clamp(0.0, 100.0) as u64;
        (latency > 0).then(|| std::time::Duration::from_millis(latency))
    });

    // Set other parameters as needed
    traffic_shaper.set_drop_probability(0.0);
    traffic_shaper.set_corrupt_probability(0.0);
    traffic_shaper.set_bandwidth_limit(100);

    // Spawn a task to send packets into the TrafficShaper
    tokio::spawn(async move {
        for i in 0..10usize {
            let mut data = Vec::new();
            let now = Utc::now().timestamp_nanos_opt().unwrap();
            data.extend_from_slice(&now.to_le_bytes());
            data.extend_from_slice(&i.to_le_bytes());
            tx.send(Bytes::from(data)).unwrap();
        }
    });

    // Consume the shaped traffic

    let output = traffic_shaper
        .map(|packet| {
            let now = Utc::now().timestamp_nanos_opt().unwrap();
            let then = Duration::nanoseconds(i64::from_le_bytes(packet[0..8].try_into().unwrap()));
            let i = usize::from_le_bytes(packet[8..16].try_into().unwrap());
            let delta = Duration::nanoseconds(now - then.num_nanoseconds().unwrap());
            println!("{i}");
            (i, delta)
        })
        .collect::<Vec<_>>()
        .await;

    // output.sort_by_key(|(i, _)| *i);

    for (i, delta) in output {
        println!("[{i}] {}ms", delta.num_milliseconds());
    }
}
