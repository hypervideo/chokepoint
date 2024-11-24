use chokepoint::ChokeItem;
use chrono::prelude::*;
use std::time::Duration;

#[derive(Debug)]
pub struct TestPayload {
    pub created: DateTime<Utc>,
    pub i: usize,
    pub size: usize,
}

impl std::fmt::Display for TestPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Payload(time={}, i={})", self.created.to_rfc3339(), self.i)
    }
}

impl TestPayload {
    pub fn new(i: usize, size: usize) -> Self {
        Self {
            created: Utc::now(),
            size,
            i,
        }
    }

    pub fn elapsed(&self) -> Duration {
        Utc::now().signed_duration_since(self.created).to_std().unwrap()
    }
}

impl ChokeItem for TestPayload {
    fn byte_len(&self) -> usize {
        self.size
    }

    fn corrupt(&mut self) {
        todo!()
    }
}
