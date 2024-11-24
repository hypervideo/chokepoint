use chokepoint::{
    normal_distribution,
    ChokeSettings,
    ChokeSettingsOrder,
    ChokeSink,
};
use chokepoint_test_helpers::*;
use futures::SinkExt as _;

#[tokio::test]
async fn unchanged() {
    let mut sink = ChokeSink::new(TestSink::default(), Default::default());

    for i in 0..10usize {
        sink.send(TestPayload::new(i, 1)).await.unwrap();
    }

    sink.close().await.unwrap();

    let received = sink
        .into_inner()
        .received
        .into_inner()
        .into_iter()
        .map(|(_, TestPayload { i, .. })| i)
        .collect::<Vec<_>>();

    assert_eq!(received.len(), 10);
    assert_eq!(received, (0..10).collect::<Vec<_>>());
}

#[tokio::test]
async fn let_it_sink_in() {
    let mut sink = ChokeSink::new(
        TestSink::default(),
        ChokeSettings::default()
            .set_latency_distribution(normal_distribution(5.0, 10.0, 100.0))
            .set_ordering(Some(ChokeSettingsOrder::Unordered)),
    );

    for i in 0..10usize {
        sink.send(TestPayload::new(i, 1)).await.unwrap();
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

#[tokio::test]
async fn sink_with_a_hole() {
    let mut sink = ChokeSink::new(
        TestSink::default(),
        ChokeSettings::default().set_drop_probability(Some(0.5)),
    );

    for i in 0..10usize {
        sink.send(TestPayload::new(i, 1)).await.unwrap();
    }

    sink.close().await.unwrap();

    let received = sink
        .into_inner()
        .received
        .into_inner()
        .into_iter()
        .map(|(_, TestPayload { i, .. })| i)
        .collect::<Vec<_>>();

    assert!(received.len() < 10);
}
