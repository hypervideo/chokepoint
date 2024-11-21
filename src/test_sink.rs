use crate::ChokeItem;
use chrono::prelude::*;
use futures::Sink;
use std::{
    pin::Pin,
    task::{
        Context,
        Poll,
    },
    time::Duration,
};

#[derive(Debug)]
pub struct TestPayload {
    pub created: DateTime<Utc>,
    pub i: usize,
}

impl std::fmt::Display for TestPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Payload(time={}, i={})", self.created.to_rfc3339(), self.i)
    }
}

impl TestPayload {
    pub fn new(i: usize) -> Self {
        Self { created: Utc::now(), i }
    }

    pub fn elapsed(&self) -> Duration {
        Utc::now().signed_duration_since(self.created).to_std().unwrap()
    }
}

impl ChokeItem for TestPayload {
    fn byte_len(&self) -> usize {
        8 + 8
    }

    fn corrupt(&mut self) {
        todo!()
    }
}

#[derive(Default)]
pub struct TestSink {
    pub received: std::cell::RefCell<Vec<(DateTime<Utc>, TestPayload)>>,
}

impl Sink<TestPayload> for TestSink {
    type Error = ();

    fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        trace!("poll_ready");
        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, item: TestPayload) -> Result<(), Self::Error> {
        trace!("[{item}] received");
        self.received.borrow_mut().push((Utc::now(), item));
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
