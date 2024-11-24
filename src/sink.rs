use crate::{
    item::ChokeItem,
    ChokeSettings,
    ChokeSettingsOrder,
    ChokeStream,
};
use futures::{
    Sink,
    SinkExt,
    StreamExt,
};
use std::{
    pin::Pin,
    task::{
        Context,
        Poll,
    },
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;

const VERBOSE: bool = false;

/// A [`futures::Sink`] that uses an underlaying [`ChokeStream`] to control how items are forwarded to the inner sink.
#[allow(clippy::type_complexity)]
#[pin_project]
pub struct ChokeSink<Si, T>
where
    Si: Sink<T> + Unpin,
{
    /// The inner sink that gets written to.
    sink: Si,
    /// The choke stream that controls how items are forwarded to the inner sink.
    choke_stream: ChokeStream<T>,
    sender: mpsc::UnboundedSender<T>,
    backpressure: bool,
}

impl<Si, T> ChokeSink<Si, T>
where
    Si: Sink<T> + Unpin,
    T: ChokeItem,
{
    pub fn new(sink: Si, settings: ChokeSettings) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let stream = Box::new(UnboundedReceiverStream::new(rx));
        Self {
            sink,
            sender: tx,
            backpressure: settings.ordering.unwrap_or_default() == ChokeSettingsOrder::Backpressure,
            choke_stream: ChokeStream::new(stream, settings),
        }
    }

    pub fn into_inner(self) -> Si {
        self.sink
    }
}

impl<Si, T> Sink<T> for ChokeSink<Si, T>
where
    Si: Sink<T> + Unpin + 'static,
    T: ChokeItem + Send + 'static,
{
    type Error = Si::Error;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if VERBOSE {
            debug!(backpressure = %self.backpressure, pending = %self.choke_stream.pending(), "poll_ready");
        }
        if self.backpressure && self.choke_stream.pending() {
            return Poll::Pending;
        }
        self.sink.poll_ready_unpin(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: T) -> Result<(), Self::Error> {
        if VERBOSE {
            debug!(pending = %self.choke_stream.pending(), "start_send");
        }
        self.sender.send(item).expect("the stream owns the receiver");
        Ok(())
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if VERBOSE {
            debug!(pending = %self.choke_stream.pending(), "poll_flush");
        }

        match self.choke_stream.poll_next_unpin(cx) {
            Poll::Ready(Some(item)) => {
                if VERBOSE {
                    debug!(pending = %self.choke_stream.pending(), "poll_flush: got item");
                }
                match self.sink.start_send_unpin(item) {
                    Ok(()) => self.sink.poll_flush_unpin(cx),
                    Err(err) => Poll::Ready(Err(err)),
                }
            }
            Poll::Ready(None) => Poll::Ready(Ok(())),
            Poll::Pending => {
                if self.choke_stream.has_dropped_item() {
                    self.choke_stream.reset_dropped_item();
                    Poll::Ready(Ok(()))
                } else {
                    Poll::Pending
                }
            }
        }
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if VERBOSE {
            debug!(pending = %self.choke_stream.pending(), "poll_close");
        }

        if self.choke_stream.pending() {
            if let Poll::Ready(Err(err)) = self.poll_flush(cx) {
                return Poll::Ready(Err(err));
            };
            Poll::Pending
        } else {
            self.sink.poll_close_unpin(cx)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        normal_distribution,
        test_sink::{
            TestPayload,
            TestSink,
        },
    };

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
}
