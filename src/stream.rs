use crate::payload::ChokeItem;
use burster::{
    Limiter,
    SlidingWindowCounter,
};
use futures::{
    Stream,
    StreamExt,
};
use rand::Rng;
#[cfg(not(target_arch = "wasm32"))]
use std::time::{
    SystemTime,
    UNIX_EPOCH,
};
use std::{
    collections::VecDeque,
    pin::Pin,
    task::{
        Context,
        Poll,
    },
    time::Duration,
};
use tokio::sync::mpsc;
#[cfg(not(target_arch = "wasm32"))]
use tokio::time::{
    interval,
    Interval,
};
#[cfg(not(target_arch = "wasm32"))]
use tokio_util::time::DelayQueue;
#[cfg(target_arch = "wasm32")]
use wasmtimer::{
    std::{
        SystemTime,
        UNIX_EPOCH,
    },
    tokio::{
        interval,
        Interval,
    },
    tokio_util::DelayQueue,
};

/// Settings for1
#[derive(Default)]
#[allow(clippy::type_complexity)]
pub struct ChokeStreamSettings {
    latency_distribution: Option<Option<Box<dyn FnMut() -> Option<Duration> + Send + Sync>>>,
    drop_probability: Option<f64>,
    corrupt_probability: Option<f64>,
    duplicate_probability: Option<f64>,
    bandwidth_limiter: Option<Option<SlidingWindowCounter<fn() -> Duration>>>,
    settings_rx: Option<mpsc::Receiver<ChokeStreamSettings>>,
}

impl ChokeStreamSettings {
    pub fn settings_updater(&mut self) -> mpsc::Sender<ChokeStreamSettings> {
        let (settings_tx, settings_rx) = mpsc::channel(1);
        self.settings_rx = Some(settings_rx);
        settings_tx
    }

    /// Set the bandwidth limit in bytes per second.
    pub fn set_bandwidth_limit(mut self, bytes_per_seconds: usize) -> Self {
        self.bandwidth_limiter = Some(Some(SlidingWindowCounter::new_with_time_provider(
            bytes_per_seconds as _,
            1000, /* ms */
            || SystemTime::now().duration_since(UNIX_EPOCH).unwrap(),
        )));
        self
    }

    /// Set the latency distribution function.
    pub fn set_latency_distribution<F>(mut self, f: F) -> Self
    where
        F: FnMut() -> Option<Duration> + Send + Sync + 'static,
    {
        self.latency_distribution = Some(Some(Box::new(f)));
        self
    }

    /// Set the probability of packet drop (0.0 to 1.0).
    pub fn set_drop_probability(mut self, probability: f64) -> Self {
        self.drop_probability = Some(probability);
        self
    }

    /// Set the probability of packet corruption (0.0 to 1.0).
    pub fn set_corrupt_probability(mut self, probability: f64) -> Self {
        self.corrupt_probability = Some(probability);
        self
    }

    /// Set the probability of packet duplication (0.0 to 1.0).
    pub fn set_duplicate_probability(mut self, probability: f64) -> Self {
        self.duplicate_probability = Some(probability);
        self
    }
}

/// A traffic shaper that can simulate various network conditions.
///
/// Example:
/// ```rust
/// # use chokepoint::{normal_distribution, ChokeStream, ChokeStreamSettings};
/// # use bytes::Bytes;
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
/// let mut traffic_shaper = ChokeStream::new(
///     Box::new(UnboundedReceiverStream::new(rx)),
///     ChokeStreamSettings::default()
///         // Set the latency distribution in milliseconds
///         .set_latency_distribution(normal_distribution(
///             10.0,  /* mean */
///             15.0,  /* stddev */
///             100.0, /* max */
///         ))
///         // Set other parameters as needed
///         .set_drop_probability(0.0)
///         .set_corrupt_probability(0.0)
///         .set_bandwidth_limit(100 /* bytes per second */),
/// );
///
/// // Spawn a task to send packets into the ChokeStream
/// tokio::spawn(async move {
///     for i in 0..10usize {
///         let mut data = Vec::new();
///         let now = Utc::now().timestamp_nanos_opt().unwrap();
///         data.extend_from_slice(&now.to_le_bytes());
///         data.extend_from_slice(&i.to_le_bytes());
///         println!("[{i}] emitting");
///         tx.send(Bytes::from(data)).unwrap();
///     }
/// });
///
/// // Consume packets from the stream
/// while let Some(packet) = traffic_shaper.next().await {
///     let now = Utc::now().timestamp_nanos_opt().unwrap();
///     let then = Duration::nanoseconds(i64::from_le_bytes(packet[0..8].try_into().unwrap()));
///     let i = usize::from_le_bytes(packet[8..16].try_into().unwrap());
///     let delta = Duration::nanoseconds(now - then.num_nanoseconds().unwrap());
///     println!("[{i}] received after {}ms", delta.num_milliseconds());
/// }
/// # }
/// ```
pub struct ChokeStream<T> {
    stream: Box<dyn Stream<Item = T> + Unpin>,
    queue: VecDeque<T>,
    delay_queue: DelayQueue<T>,
    latency_distribution: Option<Box<dyn FnMut() -> Option<Duration> + Send + Sync>>,
    drop_probability: f64,
    corrupt_probability: f64,
    duplicate_probability: f64,
    bandwidth_limiter: Option<SlidingWindowCounter<fn() -> Duration>>,
    bandwidth_timer: Option<Interval>,
    settings_rx: Option<mpsc::Receiver<ChokeStreamSettings>>,
}

impl<T> ChokeStream<T> {
    pub fn new(stream: Box<dyn Stream<Item = T> + Unpin>, settings: ChokeStreamSettings) -> Self {
        let mut stream = ChokeStream {
            stream,
            queue: VecDeque::new(),
            delay_queue: DelayQueue::new(),
            latency_distribution: None,
            drop_probability: 0.0,
            corrupt_probability: 0.0,
            duplicate_probability: 0.0,
            bandwidth_limiter: None,
            bandwidth_timer: None,
            settings_rx: None,
        };
        stream.apply_settings(settings);
        stream
    }

    pub fn apply_settings(&mut self, settings: ChokeStreamSettings) {
        if let Some(settings_rx) = settings.settings_rx {
            self.settings_rx = Some(settings_rx);
        }
        if let Some(latency_distribution) = settings.latency_distribution {
            self.latency_distribution = latency_distribution;
        }
        if let Some(drop_probability) = settings.drop_probability {
            self.drop_probability = drop_probability;
        }
        if let Some(corrupt_probability) = settings.corrupt_probability {
            self.corrupt_probability = corrupt_probability;
        }
        if let Some(duplicate_probability) = settings.duplicate_probability {
            self.duplicate_probability = duplicate_probability;
        }
        if let Some(bandwidth_limiter) = settings.bandwidth_limiter {
            self.bandwidth_limiter = bandwidth_limiter;
            if self.bandwidth_limiter.is_some() {
                self.bandwidth_timer = Some(interval(Duration::from_millis(100)));
            }
        }
    }
}

impl<T> Stream for ChokeStream<T>
where
    T: ChokeItem,
{
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        let mut rng = rand::thread_rng();

        if let Some(new_settings) = this.settings_rx.as_mut().and_then(|s| s.try_recv().ok()) {
            this.apply_settings(new_settings);
        }

        // First, take packets from the receiver and process them.
        loop {
            match (
                this.stream.poll_next_unpin(cx),
                this.delay_queue.poll_expired(cx),
                this.bandwidth_timer.as_mut().map(|timer| timer.poll_tick(cx)),
            ) {
                (Poll::Ready(Some(mut packet)), _, _) => {
                    // Simulate packet loss
                    if rng.gen::<f64>() < this.drop_probability {
                        continue;
                    }

                    // Simulate packet corruption
                    if rng.gen::<f64>() < this.corrupt_probability {
                        packet.corrupt();
                    }

                    // Simulate latency using the user-defined distribution
                    let delay = this.latency_distribution.as_mut().and_then(|latency_fn| latency_fn());

                    // Simulate packet duplication
                    if rng.gen::<f64>() < this.duplicate_probability {
                        if let Some(packet) = packet.duplicate() {
                            this.queue.push_back(packet);
                        } else {
                            tracing::warn!("Failed to duplicate packet");
                        }
                    }

                    // Insert the packet into the DelayQueue with the calculated delay
                    if let Some(delay) = delay {
                        this.delay_queue.insert(packet, delay);
                    } else {
                        this.queue.push_back(packet);
                    }
                }

                (_, Poll::Ready(Some(expired)), _) => {
                    this.queue.push_back(expired.into_inner());
                }

                (Poll::Ready(None), _, _) if this.delay_queue.is_empty() && this.queue.is_empty() => {
                    return Poll::Ready(None);
                }

                _ => {
                    // No more packets to read at the moment
                    break;
                }
            }
        }

        // Retrieve packets from the normal or delay queue
        if let Some(packet) = this.queue.pop_front() {
            // Simulate bandwidth limitation
            if let (Some(limiter), Some(timer)) = (&mut this.bandwidth_limiter, &mut this.bandwidth_timer) {
                if limiter.try_consume(packet.byte_len() as _).is_err() {
                    // If the packet cannot be sent due to bandwidth limitation, reinsert it into the queue
                    this.queue.push_front(packet);
                    timer.reset(); // to wake up later without busy waking
                    return Poll::Pending;
                }
            }
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
}
