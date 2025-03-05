use crate::time::Instant;
use std::{
    collections::VecDeque,
    time::Duration,
};

#[derive(Debug, Default)]
pub struct BandwidthLimiter {
    limit: usize,
    current_burden: usize,
    requests: VecDeque<(Instant, usize)>,
    window: Duration,
}

impl BandwidthLimiter {
    pub fn new(limit: usize, window: Duration) -> Self {
        Self {
            limit,
            window,
            ..Default::default()
        }
    }

    pub fn limit_reached(&self) -> bool {
        self.capacity_left() == 0
    }

    pub fn capacity_left(&self) -> usize {
        self.limit.saturating_sub(self.current_burden)
    }

    #[allow(dead_code)]
    pub fn deadline(&self) -> Option<Instant> {
        self.requests.front().map(|(time, _)| *time + self.window)
    }

    pub fn deadline_duration(&self, now: Instant) -> Option<Duration> {
        self.requests.front().and_then(|(time, _)| {
            let deadline = *time + self.window;
            deadline.checked_duration_since(now)
        })
    }

    pub fn update_at(&mut self, now: Instant) {
        let cutoff = now - self.window;
        let n = self.requests.iter().take_while(|(time, _)| *time < cutoff).count();
        if n == 0 {
            return;
        };
        let remaining = self.requests.split_off(n);
        for (_, weight) in self.requests.drain(..) {
            self.current_burden -= weight;
        }
        self.requests = remaining;
    }

    pub fn add_request(&mut self, weight: usize) {
        self.add_request_at(weight, Instant::now())
    }

    pub fn add_request_at(&mut self, weight: usize, now: Instant) {
        self.requests.push_back((now, weight));
        self.current_burden += weight;
        self.update_at(now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_based_capacity_window() {
        let mut limiter = BandwidthLimiter::new(10, Duration::from_secs(1));
        assert_eq!(limiter.capacity_left(), 10);

        let now = Instant::now();
        limiter.add_request_at(5, now);
        assert_eq!(limiter.capacity_left(), 5);

        let now = now + Duration::from_millis(500);
        limiter.add_request_at(5, now);
        assert_eq!(limiter.capacity_left(), 0);
        limiter.add_request(1); // We allow to still add more requests

        let now = now + Duration::from_millis(400);
        let duration = limiter.deadline_duration(now).unwrap();
        assert!(
            duration <= Duration::from_millis(100) && duration >= Duration::from_millis(90),
            "{:?}",
            duration
        );

        let now = now + Duration::from_millis(200);
        limiter.update_at(now);
        assert_eq!(limiter.capacity_left(), 4);
    }
}
