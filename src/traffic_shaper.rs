use burster::{Limiter, SlidingWindowCounter};
use futures::{Stream, StreamExt};
use rand::Rng;
use std::collections::VecDeque;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::time::Duration;
use tokio_util::time::DelayQueue;

use crate::payload::TrafficShaperPayload;

/// A traffic shaper that can simulate various network conditions.
///
/// Example:
/// ```rust
/// # use bytes::Bytes;
/// # use chokepoint::TrafficShaper;
/// # use chrono::{prelude::*, Duration};
/// # use futures::stream::StreamExt;
/// # use rand_distr::{num_traits::Float as _, Distribution as _, Normal};
/// # use tokio::sync::mpsc;
/// # use tokio_stream::wrappers::UnboundedReceiverStream;
/// #
/// # #[tokio::main]
/// # async fn main() {
/// let (tx, rx) = mpsc::unbounded_channel();
///
/// let mut traffic_shaper = TrafficShaper::new(Box::new(UnboundedReceiverStream::new(rx)));
///
/// // Set the latency distribution
/// traffic_shaper.set_latency_distribution(|| {
///     let normal = Normal::new(10.0, 15.0).unwrap(); // mean = 10ms, std dev = 15ms
///     let latency = normal.sample(&mut rand::thread_rng()).clamp(0.0, 100.0) as u64;
///     (latency > 0).then(|| std::time::Duration::from_millis(latency))
/// });
///
/// // Set other parameters as needed
/// traffic_shaper.set_drop_probability(0.0);
/// traffic_shaper.set_corrupt_probability(0.0);
/// traffic_shaper.set_bandwidth_limit(100);
///
/// // Spawn a task to send packets into the TrafficShaper
/// tokio::spawn(async move {
///     for i in 0..10usize {
///         let mut data = Vec::new();
///         let now = Utc::now().timestamp_nanos_opt().unwrap();
///         data.extend_from_slice(&now.to_le_bytes());
///         data.extend_from_slice(&i.to_le_bytes());
///         tx.send(Bytes::from(data)).unwrap();
///     }
/// });
///
/// // Consume the shaped traffic
///
/// let output = traffic_shaper
///     .map(|packet| {
///         let now = Utc::now().timestamp_nanos_opt().unwrap();
///         let then = Duration::nanoseconds(i64::from_le_bytes(packet[0..8].try_into().unwrap()));
///         let i = usize::from_le_bytes(packet[8..16].try_into().unwrap());
///         let delta = Duration::nanoseconds(now - then.num_nanoseconds().unwrap());
///         println!("{i}");
///         (i, delta)
///     })
///     .collect::<Vec<_>>()
///     .await;
///
/// // output.sort_by_key(|(i, _)| *i);
///
/// for (i, delta) in output {
///     println!("[{i}] {}ms", delta.num_milliseconds());
/// }
/// # }
/// ```
pub struct TrafficShaper<T> {
    stream: Box<dyn Stream<Item = T> + Unpin>,
    queue: VecDeque<T>,
    delay_queue: DelayQueue<T>,
    latency_distribution: Option<Box<dyn FnMut() -> Option<Duration> + Send + Sync>>,
    drop_probability: f64,
    corrupt_probability: f64,
    bandwidth_limiter: Option<SlidingWindowCounter<fn() -> std::time::Duration>>,
}

impl<T> TrafficShaper<T> {
    pub fn new(stream: Box<dyn Stream<Item = T> + Unpin>) -> Self {
        TrafficShaper {
            stream,
            queue: VecDeque::new(),
            delay_queue: DelayQueue::new(),
            latency_distribution: None,
            drop_probability: 0.0,
            corrupt_probability: 0.0,
            bandwidth_limiter: None,
        }
    }

    /// Set the bandwidth limit in bytes per second.
    pub fn set_bandwidth_limit(&mut self, bytes_per_seconds: usize) {
        self.bandwidth_limiter = Some(SlidingWindowCounter::new_with_time_provider(
            bytes_per_seconds as _,
            1000, /*ms*/
            || {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
            },
        ));
    }

    /// Set the latency distribution function.
    pub fn set_latency_distribution<F>(&mut self, f: F)
    where
        F: FnMut() -> Option<Duration> + Send + Sync + 'static,
    {
        self.latency_distribution = Some(Box::new(f));
    }

    /// Set the probability of packet drop (0.0 to 1.0).
    pub fn set_drop_probability(&mut self, probability: f64) {
        self.drop_probability = probability;
    }

    /// Set the probability of packet corruption (0.0 to 1.0).
    pub fn set_corrupt_probability(&mut self, probability: f64) {
        self.corrupt_probability = probability;
    }
}

impl<T> Stream for TrafficShaper<T>
where
    T: TrafficShaperPayload,
{
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        let mut rng = rand::thread_rng();

        // First, take packets from the receiver and process them.
        loop {
            match (
                this.stream.poll_next_unpin(cx),
                this.delay_queue.poll_expired(cx),
            ) {
                (Poll::Ready(Some(packet)), _) => {
                    // Simulate latency using the user-defined distribution
                    let delay = this
                        .latency_distribution
                        .as_mut()
                        .and_then(|latency_fn| latency_fn());

                    // Insert the packet into the DelayQueue with the calculated delay
                    if let Some(delay) = delay {
                        this.delay_queue.insert(packet, delay);
                    } else {
                        this.queue.push_back(packet);
                    }
                }

                (_, Poll::Ready(Some(expired))) => {
                    this.queue.push_back(expired.into_inner());
                }

                (Poll::Ready(None), _) if this.delay_queue.is_empty() && this.queue.is_empty() => {
                    return Poll::Ready(None);
                }

                _ => {
                    // No more packets to read at the moment
                    break;
                }
            }
        }

        // Retrieve packets from the normal or delay queue
        while let Some(mut packet) = this.queue.pop_front() {
            // Simulate packet loss
            if rng.gen::<f64>() < this.drop_probability {
                continue;
            }

            // Simulate packet corruption
            if rng.gen::<f64>() < this.corrupt_probability {
                packet.corrupt();
            }

            // Simulate bandwidth limitation by adjusting the delay
            if let Some(limiter) = &mut this.bandwidth_limiter {
                if limiter.try_consume(packet.byte_len() as _).is_err() {
                    // If the packet cannot be sent due to bandwidth limitation, reinsert it into the queue
                    // this.delay_queue.insert(packet, Duration::from_millis(10));
                    this.queue.push_front(packet);
                    continue;
                }
            };

            return Poll::Ready(Some(packet));
        }

        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use bytes::Bytes;
    use futures::stream::StreamExt;
    use tokio::sync::mpsc;
    use tokio_stream::wrappers::UnboundedReceiverStream;

    #[tokio::test]
    async fn delivery_without_modifications() {
        let (tx, rx) = mpsc::unbounded_channel();
        let traffic_shaper = TrafficShaper::new(Box::new(UnboundedReceiverStream::new(rx)));

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
}
