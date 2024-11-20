use chokepoint::{
    normal_distribution,
    ChokeItem,
    ChokeSettings,
    ChokeSink,
};
use futures::{
    Sink,
    SinkExt,
};
use std::{
    pin::Pin,
    task::{
        Context,
        Poll,
    },
};

#[macro_use]
extern crate tracing;

#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
struct N(usize);

impl std::fmt::Display for N {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl std::ops::Deref for N {
    type Target = usize;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ChokeItem for N {
    fn byte_len(&self) -> usize {
        8
    }

    fn corrupt(&mut self) {
        todo!()
    }
}

#[derive(Default)]
struct TracingSink {
    received: std::cell::RefCell<Vec<N>>,
}

impl Sink<N> for TracingSink {
    type Error = ();

    fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        trace!("poll_ready");
        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, item: N) -> Result<(), Self::Error> {
        println!("[{item}] received");
        self.received.borrow_mut().push(item);
        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        trace!("poll_flush");
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        trace!("poll_close");
        Poll::Ready(Ok(()))
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::builder().parse_lossy("trace"))
        .with_span_events(
            tracing_subscriber::fmt::format::FmtSpan::NEW | tracing_subscriber::fmt::format::FmtSpan::CLOSE,
        )
        .init();

    let settings = ChokeSettings::default().set_latency_distribution(normal_distribution(20.0, 100.0, 1000.0));

    let mut sink = ChokeSink::new(TracingSink::default(), settings);

    for i in 0..10usize {
        println!("[{i}] emitting");
        sink.send(N(i)).await.unwrap();
    }

    println!("closing sink");
    sink.close().await.unwrap();

    let received = sink.into_inner().received.into_inner();
    println!("received: {:?}", received);
}
