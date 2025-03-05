use crate::{
    item::ChokeItem,
    settings::BandwidthLimit,
    time::{
        tokio_time::{
            interval,
            Interval,
        },
        Instant,
    },
    ChokeSettings,
    ChokeSettingsOrder,
};
use futures::{
    Stream,
    StreamExt,
};
use rand::Rng;
use std::{
    collections::{
        BTreeMap,
        VecDeque,
    },
    pin::Pin,
    task::{
        Context,
        Poll,
    },
    time::Duration,
};
use tokio::sync::mpsc;

const VERBOSE: bool = false;

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
///         .set_bandwidth_limit(Some(100 /* bytes per second */), 0.1 /* drop prob */, true /* only drop when limit reached */),
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
    queue: Queue<T>,
    latency_distribution: Option<Box<dyn FnMut() -> Option<Duration> + Send + Sync>>,
    drop_probability: f64,
    corrupt_probability: f64,
    duplicate_probability: f64,
    bandwidth_limit: Option<BandwidthLimit>,
    timer: Interval,
    ordering: ChokeSettingsOrder,
    settings_rx: Option<mpsc::Receiver<ChokeSettings>>,
    has_dropped_item: bool,
    total_packets: usize,
    dropped_packets: usize,
    packets_per_second: usize,
    debug_timer: Interval,
}

impl<T> ChokeStream<T> {
    pub fn new(stream: Box<dyn Stream<Item = T> + Unpin>, settings: ChokeSettings) -> Self {
        if VERBOSE {
            debug!(?settings, "creating new ChokeStream");
        }
        let ordering = settings.ordering.unwrap_or_default();
        let mut stream = ChokeStream {
            stream,
            queue: Queue::queue_for_ordering(ordering),
            latency_distribution: None,
            drop_probability: 0.0,
            corrupt_probability: 0.0,
            duplicate_probability: 0.0,
            bandwidth_limit: None,
            timer: interval(Duration::from_millis(20)),
            ordering,
            settings_rx: None,
            has_dropped_item: false,
            total_packets: 0,
            dropped_packets: 0,
            packets_per_second: 0,
            debug_timer: interval(Duration::from_secs_f64(2.5)),
        };
        stream.apply_settings(settings);
        stream
    }

    pub fn apply_settings(&mut self, settings: ChokeSettings) {
        debug!(?settings, "applying settings");

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
        if let Some(ordering) = settings.ordering {
            self.ordering = ordering;
            self.queue = Queue::queue_for_ordering(ordering);
        }
        if let Some(bandwidth_limit) = settings.bandwidth_limit {
            self.bandwidth_limit = bandwidth_limit;
        }
    }

    pub(crate) fn pending(&self) -> bool {
        self.queue.pending()
    }

    pub(crate) fn has_dropped_item(&self) -> bool {
        self.has_dropped_item
    }

    pub(crate) fn reset_dropped_item(&mut self) {
        self.has_dropped_item = false;
    }

    fn backpressure(&self) -> bool {
        self.ordering == ChokeSettingsOrder::Backpressure
    }
}

enum Queue<T> {
    Unordered(UnorderedQueue<T>),
    Ordered(OrderedQueue<T>),
}

impl<T> Queue<T> {
    fn queue_for_ordering(ordering: ChokeSettingsOrder) -> Self {
        match ordering {
            ChokeSettingsOrder::Ordered => Queue::Ordered(OrderedQueue {
                queue: VecDeque::new(),
                delayed: 0,
            }),
            ChokeSettingsOrder::Unordered | ChokeSettingsOrder::Backpressure => Queue::Unordered(UnorderedQueue {
                queue: VecDeque::new(),
                delay_queue: BTreeMap::new(),
            }),
        }
    }

    fn queued(&self) -> usize {
        match self {
            Queue::Unordered(q) => q.queued(),
            Queue::Ordered(q) => q.queued(),
        }
    }

    fn delayed(&self) -> usize {
        match self {
            Queue::Unordered(q) => q.delayed(),
            Queue::Ordered(q) => q.delayed(),
        }
    }

    fn pending(&self) -> bool {
        match self {
            Queue::Unordered(q) => q.pending(),
            Queue::Ordered(q) => q.pending(),
        }
    }

    fn deadline(&self) -> Option<Instant> {
        match self {
            Queue::Unordered(q) => q.deadline(),
            Queue::Ordered(q) => q.deadline(),
        }
    }

    fn expire(&mut self, now: Instant) {
        match self {
            Queue::Unordered(q) => q.expire(now),
            Queue::Ordered(_) => {}
        }
    }

    fn pop_front(&mut self, now: Instant) -> Option<T> {
        match self {
            Queue::Unordered(q) => q.pop_front(),
            Queue::Ordered(q) => q.pop_front(now),
        }
    }

    fn push_front(&mut self, item: T, delay: Option<Duration>, now: Instant) {
        match self {
            Queue::Unordered(q) => q.push_front(item, delay, now),
            Queue::Ordered(q) => q.push(true, item, delay, now),
        }
    }

    fn push_back(&mut self, item: T, delay: Option<Duration>, now: Instant) {
        match self {
            Queue::Unordered(q) => q.push_back(item, delay, now),
            Queue::Ordered(q) => q.push(false, item, delay, now),
        }
    }
}

struct UnorderedQueue<T> {
    queue: VecDeque<T>,
    delay_queue: BTreeMap<Instant, T>,
}

impl<T> UnorderedQueue<T> {
    fn queued(&self) -> usize {
        self.queue.len()
    }

    fn delayed(&self) -> usize {
        self.delay_queue.len()
    }

    fn pending(&self) -> bool {
        !self.queue.is_empty() || !self.delay_queue.is_empty()
    }

    fn deadline(&self) -> Option<Instant> {
        self.delay_queue.keys().next().copied()
    }

    fn expire(&mut self, now: Instant) {
        let still_delayed = self.delay_queue.split_off(&now);
        let expired = std::mem::replace(&mut self.delay_queue, still_delayed);
        self.queue.extend(expired.into_values());
    }

    fn pop_front(&mut self) -> Option<T> {
        self.queue.pop_front()
    }

    fn push_front(&mut self, item: T, delay: Option<Duration>, now: Instant) {
        if let Some(delay) = delay {
            let instant = now + delay;
            self.delay_queue.insert(instant, item);
        } else {
            self.queue.push_front(item);
        }
    }

    fn push_back(&mut self, item: T, delay: Option<Duration>, now: Instant) {
        if let Some(delay) = delay {
            let instant = now + delay;
            self.delay_queue.insert(instant, item);
        } else {
            self.queue.push_back(item);
        }
    }
}

struct OrderedQueue<T> {
    queue: VecDeque<(Option<Instant>, T)>,
    delayed: usize,
}

impl<T> OrderedQueue<T> {
    fn queued(&self) -> usize {
        self.queue.len()
    }

    fn delayed(&self) -> usize {
        self.delayed
    }

    fn pending(&self) -> bool {
        !self.queue.is_empty()
    }

    fn deadline(&self) -> Option<Instant> {
        self.queue.front().and_then(|(instant, _)| *instant)
    }

    fn pop_front(&mut self, now: Instant) -> Option<T> {
        match self.queue.front() {
            Some((Some(instant), _)) if *instant > now => None,
            Some((Some(_), _)) => {
                self.delayed -= 1;
                self.queue.pop_front().map(|(_, item)| item)
            }
            _ => self.queue.pop_front().map(|(_, item)| item),
        }
    }

    fn push(&mut self, front: bool, item: T, delay: Option<Duration>, now: Instant) {
        let item = if let Some(delay) = delay {
            self.delayed += 1;
            (Some(now + delay), item)
        } else {
            (None, item)
        };
        if front {
            self.queue.push_front(item);
        } else {
            self.queue.push_back(item)
        };
    }
}

impl<T> Stream for ChokeStream<T>
where
    T: ChokeItem,
{
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if VERBOSE {
            debug!(
                queued = self.queue.queued(),
                delayed = self.queue.delayed(),
                "poll_next"
            );
        }

        let this = self.get_mut();

        if let Some(new_settings) = this.settings_rx.as_mut().and_then(|s| s.try_recv().ok()) {
            debug!(?new_settings, "settings changed");
            this.apply_settings(new_settings);
        }

        if this.debug_timer.poll_tick(cx).is_ready() {
            this.debug_timer.reset();
            debug!(
                queued = this.queue.queued(),
                delayed = this.queue.delayed(),
                packets_per_second = %this.packets_per_second,
                total_packets = %this.total_packets,
                dropped_packets = %this.dropped_packets,
                ordering = ?this.ordering,
                "packets per second"
            );
            this.packets_per_second = 0;
        }

        let now = Instant::now();
        let mut rng = rand::thread_rng();

        // First, take packets from the receiver and process them.
        if !this.backpressure() || !this.queue.pending() {
            if VERBOSE {
                debug!("waiting for packets from inner stream");
            }
            loop {
                match this.stream.poll_next_unpin(cx) {
                    Poll::Ready(Some(mut packet)) => {
                        if VERBOSE {
                            debug!(bytes = %packet.byte_len(), "received packet");
                        }

                        let bandwidth_drop = this.bandwidth_limit.as_mut().map_or(false, |limit| {
                            if limit.only_drop_when_bandwidth_limit_reached && !limit.window.limit_reached() {
                                return false;
                            }
                            rng.gen::<f64>() < limit.drop_ratio
                        });

                        // Simulate packet loss
                        if bandwidth_drop || rng.gen::<f64>() < this.drop_probability {
                            if VERBOSE {
                                debug!("dropped packet bandwith_drop={bandwidth_drop}");
                            }
                            this.dropped_packets += 1;
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
                        let duplicate = (rng.gen::<f64>() < this.duplicate_probability)
                            .then(|| {
                                if let Some(packet) = packet.duplicate() {
                                    if VERBOSE {
                                        debug!("duplicated packet");
                                    }
                                    Some(packet)
                                } else {
                                    warn!("Failed to duplicate packet");
                                    None
                                }
                            })
                            .flatten();

                        // Insert the packet into the DelayQueue with the calculated delay
                        this.queue.push_back(packet, delay, now);
                        if let Some(duplicate) = duplicate {
                            this.queue.push_back(duplicate, None, now);
                        }
                    }

                    Poll::Ready(None) if !this.queue.pending() => {
                        return Poll::Ready(None);
                    }

                    Poll::Ready(None) | Poll::Pending => {
                        // No more packets to read at the moment
                        break;
                    }
                }
            }
        }

        this.queue.expire(now);

        // Retrieve packets from the normal or delay queue
        if VERBOSE {
            debug!(pending = this.queue.pending(), "retrieving packet");
        }
        if let Some(packet) = this.queue.pop_front(now) {
            // debug!(pending = this.queue.len(), "packet from queue");

            // Simulate bandwidth limita
            let limit = this.bandwidth_limit.as_mut().map_or(false, |limit| {
                limit.window.update_at(now);
                if !limit.window.limit_reached() {
                    limit.window.add_request(packet.byte_len());
                    false
                } else {
                    true
                }
            });

            if limit {
                if VERBOSE {
                    debug!(i = %this.total_packets,"bandwidth limit reached");
                }
                this.queue.push_front(packet, None, now);
            } else {
                if VERBOSE {
                    debug!("emitting packet");
                }

                this.total_packets += 1;
                this.packets_per_second += 1;

                // Poll the stream again immediately for processing the next packet
                cx.waker().wake_by_ref();

                return Poll::Ready(Some(packet));
            }
        }

        if VERBOSE {
            debug!(
                queue = this.queue.queued(),
                delayed = this.queue.delayed(),
                "Poll::Pending"
            );
        }

        if this.pending() {
            let now = Instant::now();
            match this.queue.deadline() {
                Some(deadline) if deadline > now => {
                    this.timer = interval(deadline - now);
                }
                _ => {
                    this.timer = interval(Duration::from_millis(20));
                }
            }
            let _ = this.timer.poll_tick(cx);
            Poll::Pending
        } else {
            this.stream.poll_next_unpin(cx)
        }
    }
}
