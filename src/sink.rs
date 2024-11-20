use crate::{
    item::ChokeItem,
    ChokeSettings,
    ChokeStream,
};
use futures::{
    Sink,
    SinkExt,
    StreamExt,
};
use std::{
    future::Future,
    pin::Pin,
    task::{
        Context,
        Poll,
    },
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;

#[allow(clippy::type_complexity)]
pub struct ChokeSink<Si, T>
where
    Si: Sink<T> + Unpin,
{
    sink: Si,
    choke_stream: ChokeStream<T>,
    pending_send: Option<Pin<Box<dyn Future<Output = Result<(), Si::Error>>>>>,
    sender: mpsc::UnboundedSender<T>,
    closing: bool,
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
            choke_stream: ChokeStream::new(stream, settings),
            pending_send: None,
            sender: tx,
            closing: false,
        }
    }

    pub fn into_inner(self) -> Si {
        self.sink
    }
}

impl<Si, T> Sink<T> for ChokeSink<Si, T>
where
    Si: Sink<T> + Unpin,
    T: ChokeItem,
{
    type Error = Si::Error;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.sink).poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: T) -> Result<(), Self::Error> {
        if let Err(err) = self.sender.send(item) {
            error!("Failed to send item to choke stream: {err}");
        }
        Ok(())
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if let Some(mut future) = self.pending_send.take().take() {
            match future.as_mut().poll(cx) {
                Poll::Ready(Ok(())) => {}
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => {
                    self.pending_send = Some(future);
                    return Poll::Pending;
                }
            }
        }

        match self.choke_stream.poll_next_unpin(cx) {
            Poll::Ready(Some(item)) => match self.sink.poll_ready_unpin(cx) {
                Poll::Ready(Ok(())) => match (
                    self.sink.start_send_unpin(item),
                    self.closing && self.choke_stream.pending(),
                ) {
                    (Ok(()), false) => self.sink.poll_flush_unpin(cx),
                    (Ok(()), true) => {
                        cx.waker().wake_by_ref();
                        Poll::Pending
                    }
                    (Err(err), _) => Poll::Ready(Err(err)),
                },
                not_ready => not_ready,
            },
            Poll::Ready(None) => self.sink.poll_flush_unpin(cx),
            Poll::Pending => {
                if (self.closing && self.choke_stream.pending()) || self.choke_stream.pending_immediate() {
                    Poll::Pending
                } else {
                    self.sink.poll_flush_unpin(cx)
                }
            }
        }
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.closing = true;
        if self.pending_send.is_some() || self.choke_stream.pending() {
            self.poll_flush(cx)
        } else {
            self.sink.poll_close_unpin(cx)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::normal_distribution;
    use bytes::Bytes;

    #[derive(Default)]
    struct TestSink {
        received: std::cell::RefCell<Vec<Bytes>>,
    }

    impl Sink<Bytes> for TestSink {
        type Error = ();

        fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn start_send(self: Pin<&mut Self>, item: Bytes) -> Result<(), Self::Error> {
            self.received.borrow_mut().push(item);
            Ok(())
        }

        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
    }

    #[tokio::test]
    async fn unchanged() {
        let mut sink = ChokeSink::new(TestSink::default(), Default::default());

        for i in 0..10usize {
            sink.send(Bytes::from_iter(i.to_le_bytes())).await.unwrap();
        }

        sink.close().await.unwrap();

        let received = sink
            .into_inner()
            .received
            .into_inner()
            .into_iter()
            .map(|b| usize::from_le_bytes(b[..].try_into().unwrap()))
            .collect::<Vec<_>>();

        assert_eq!(received.len(), 10);
        assert_eq!(received, (0..10).collect::<Vec<_>>());
    }

    #[tokio::test]
    async fn let_it_sink_in() {
        let mut sink = ChokeSink::new(
            TestSink::default(),
            ChokeSettings::default().set_latency_distribution(normal_distribution(5.0, 10.0, 100.0)),
        );

        for i in 0..10usize {
            sink.send(Bytes::from_iter(i.to_le_bytes())).await.unwrap();
        }

        sink.close().await.unwrap();

        let mut received = sink
            .into_inner()
            .received
            .into_inner()
            .into_iter()
            .map(|b| usize::from_le_bytes(b[..].try_into().unwrap()))
            .collect::<Vec<_>>();
        received.sort();

        assert_eq!(received.len(), 10);
        assert_eq!(received, (0..10).collect::<Vec<_>>());
    }

    #[tokio::test]
    async fn sink_with_a_hole() {
        let mut sink = ChokeSink::new(TestSink::default(), ChokeSettings::default().set_drop_probability(0.5));

        for i in 0..10usize {
            sink.send(Bytes::from_iter(i.to_le_bytes())).await.unwrap();
        }

        sink.close().await.unwrap();

        let received = sink
            .into_inner()
            .received
            .into_inner()
            .into_iter()
            .map(|b| usize::from_le_bytes(b[..].try_into().unwrap()))
            .collect::<Vec<_>>();

        assert!(received.len() < 10);
    }
}
