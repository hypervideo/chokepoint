use bytes::Bytes;
use chokepoint::{
    ChokeSettings,
    ChokeSettingsOrder,
    ChokeStream,
};
use futures::stream::StreamExt;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;

#[tokio::test]
async fn delivery_without_modifications() {
    let (tx, rx) = mpsc::unbounded_channel();
    let traffic_shaper = ChokeStream::new(Box::new(UnboundedReceiverStream::new(rx)), Default::default());

    tokio::spawn(async move {
        for i in 0..10usize {
            tx.send(Bytes::from(i.to_le_bytes().to_vec())).unwrap();
        }
    });

    let output = traffic_shaper
        .map(|packet| usize::from_le_bytes(packet[0..8].try_into().unwrap()))
        .collect::<Vec<_>>()
        .await;

    assert_eq!(output, (0..10).collect::<Vec<_>>());
}

#[yare::parameterized(
        unordered = { ChokeSettingsOrder::Unordered, vec![2, 3, 1] },
        ordered = { ChokeSettingsOrder::Ordered, vec![1, 2, 3] },
        // backpressure = { ChokeSettingsOrder::Backpressure, vec![1, 2, 3] }
    )]
#[test_macro(tokio::test)]
async fn ordering(ordering: ChokeSettingsOrder, expected: Vec<usize>) {
    let (tx, rx) = mpsc::unbounded_channel();
    let stream = ChokeStream::new(
        Box::new(UnboundedReceiverStream::new(rx)),
        ChokeSettings::default()
            .set_ordering(Some(ordering))
            .set_latency_distribution(Some({
                let mut n = 0;
                move || {
                    n += 1;
                    match n {
                        1 => Some(Duration::from_millis(150)),
                        2 => Some(Duration::from_millis(50)),
                        3 => Some(Duration::from_millis(100)),
                        _ => None,
                    }
                }
            })),
    );

    tx.send(Bytes::from(1usize.to_le_bytes().to_vec())).unwrap();
    tx.send(Bytes::from(2usize.to_le_bytes().to_vec())).unwrap();
    tx.send(Bytes::from(3usize.to_le_bytes().to_vec())).unwrap();
    drop(tx);

    let output = stream
        .map(|packet| usize::from_le_bytes(packet[0..8].try_into().unwrap()))
        .collect::<Vec<_>>()
        .await;

    assert_eq!(output, expected);
}
