use crate::{
    item::ChokeItem,
    time::{
        tokio_time::{
            interval,
            Interval,
        },
        tokio_util::DelayQueue,
    },
    ChokeSettings,
};
use burster::{
    Limiter,
    SlidingWindowCounter,
};
use futures::{
    Stream,
    StreamExt,
};
use rand::Rng;
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

/// A traffic shaper that can simulate various network conditions.
///
/// Example:
/// ```rust
/// # use chokepoint::{normal_distribution, ChokeStream, ChokeSettings};
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
///     ChokeSettings::default()
///         // Set the latency distribution in milliseconds
///         .set_latency_distribution(normal_distribution(
///             10.0,  /* mean */
///             15.0,  /* stddev */
///             100.0, /* max */
///         ))
///         // Set other parameters as needed
///         .set_drop_probability(Some(0.0))
///         .set_corrupt_probability(Some(0.0))
///         .set_bandwidth_limit(Some(100 /* bytes per second */)),
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
#[pin_project]
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
    backpressure: bool,
    settings_rx: Option<mpsc::Receiver<ChokeSettings>>,
    has_dropped_item: bool,
}

impl<T> ChokeStream<T> {
    pub fn new(stream: Box<dyn Stream<Item = T> + Unpin>, settings: ChokeSettings) -> Self {
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
            backpressure: false,
            settings_rx: None,
            has_dropped_item: false,
        };
        stream.apply_settings(settings);
        stream
    }

    pub fn apply_settings(&mut self, settings: ChokeSettings) {
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
        if let Some(backpressure) = settings.backpressure {
            self.backpressure = backpressure;
        }
        if let Some(bandwidth_limiter) = settings.bandwidth_limiter {
            self.bandwidth_limiter = bandwidth_limiter;
            if self.bandwidth_limiter.is_some() {
                self.bandwidth_timer = Some(interval(Duration::from_millis(100)));
            }
        }
    }

    pub(crate) fn pending(&self) -> bool {
        !self.queue.is_empty() || !self.delay_queue.is_empty()
    }

    pub(crate) fn has_dropped_item(&self) -> bool {
        self.has_dropped_item
    }

    pub(crate) fn reset_dropped_item(&mut self) {
        self.has_dropped_item = false;
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
            debug!("settings changed");
            this.apply_settings(new_settings);
        }

        this.bandwidth_timer.as_mut().map(|timer| timer.poll_tick(cx));

        // First, take packets from the receiver and process them.
        if !this.backpressure || !this.pending() {
            loop {
                match this.stream.poll_next_unpin(cx) {
                    Poll::Ready(Some(mut packet)) => {
                        // Simulate packet loss
                        if rng.gen::<f64>() < this.drop_probability {
                            trace!("dropped");
                            this.has_dropped_item = true;
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
                                trace!("duplicated");
                                this.queue.push_back(packet);
                            } else {
                                warn!("Failed to duplicate packet");
                            }
                        }

                        // Insert the packet into the DelayQueue with the calculated delay
                        if let Some(delay) = delay {
                            debug!("delayed by {:?}", delay);
                            this.delay_queue.insert(packet, delay);
                        } else {
                            this.queue.push_back(packet);
                        }
                    }

                    Poll::Ready(None) if this.delay_queue.is_empty() && this.queue.is_empty() => {
                        return Poll::Ready(None);
                    }

                    Poll::Ready(None) | Poll::Pending => {
                        // No more packets to read at the moment
                        break;
                    }
                }
            }
        }

        while let Poll::Ready(Some(expired)) = this.delay_queue.poll_expired(cx) {
            this.queue.push_back(expired.into_inner());
        }

        // Retrieve packets from the normal or delay queue
        if let Some(packet) = this.queue.pop_front() {
            // Simulate bandwidth limitation
            if let (Some(limiter), Some(timer)) = (&mut this.bandwidth_limiter, &mut this.bandwidth_timer) {
                if limiter.try_consume(packet.byte_len() as _).is_err() {
                    trace!("bandwidth limit exceeded");
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
