use crate::TestPayload;
use chrono::prelude::*;
use futures::Sink;
use std::{
    pin::Pin,
    task::{
        Context,
        Poll,
    },
};

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
