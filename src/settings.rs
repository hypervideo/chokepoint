use burster::SlidingWindowCounter;
use std::time::Duration;
#[cfg(not(target_arch = "wasm32"))]
use std::time::{
    SystemTime,
    UNIX_EPOCH,
};
use tokio::sync::mpsc;
#[cfg(target_arch = "wasm32")]
use wasmtimer::std::{
    SystemTime,
    UNIX_EPOCH,
};

/// Settings for the [`crate::ChokeStream`] and [`crate::ChokeSink`].
// Uses double options to allow for partial updates. See `ChokeStream::apply_settings`.
#[derive(Default)]
#[allow(clippy::type_complexity)]
pub struct ChokeSettings {
    pub(crate) settings_rx: Option<mpsc::Receiver<ChokeSettings>>,
    pub(crate) latency_distribution: Option<Option<Box<dyn FnMut() -> Option<Duration> + Send + Sync>>>,
    pub(crate) drop_probability: Option<f64>,
    pub(crate) corrupt_probability: Option<f64>,
    pub(crate) duplicate_probability: Option<f64>,
    pub(crate) bandwidth_limit: Option<Option<BandwithLimit>>,
    pub(crate) ordering: Option<ChokeSettingsOrder>,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChokeSettingsOrder {
    /// Consume items as fast as possible from the inner stream. If items are delayed, their order might be changed.
    Unordered,
    /// Consume items as fast as possible from the inner stream, but ensure ordering. This is done by adjusting the
    /// delay of each item and might potentially block until a delayed item is ready.
    #[default]
    Ordered,
    /// `Backpressure` works by not consuming from the inner stream until the currently queued item has been processed.
    /// Without backpressure, the [`crate::ChokeStream`] will consume items as fast as possible.
    Backpressure,
}

pub(crate) struct BandwithLimit {
    pub(crate) window: SlidingWindowCounter<fn() -> Duration>,
    pub(crate) drop_ratio: f64,
}

impl std::fmt::Debug for BandwithLimit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BandwithLimit")
            .field("window", &"fn() -> Duration")
            .field("drop_ratio", &self.drop_ratio)
            .finish()
    }
}

impl std::fmt::Debug for ChokeSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChokeSettings")
            .field(
                "latency_distribution",
                if self.latency_distribution.is_some() {
                    &"Some"
                } else {
                    &"None"
                },
            )
            .field("drop_probability", &self.drop_probability)
            .field("corrupt_probability", &self.corrupt_probability)
            .field("duplicate_probability", &self.duplicate_probability)
            .field("bandwidth_limiter", &self.bandwidth_limit)
            .field("ordering", &self.ordering)
            .finish()
    }
}

impl ChokeSettings {
    /// Produces a [`mpsc::Sender`] that can be used to live update the configuration being used by the
    /// [`crate::ChokeStream`] / [`crate::ChokeSink`] without recreating them.
    pub fn settings_updater(&mut self) -> mpsc::Sender<ChokeSettings> {
        let (settings_tx, settings_rx) = mpsc::channel(1);
        self.settings_rx = Some(settings_rx);
        settings_tx
    }

    /// Set the bandwidth limit in bytes per second.
    pub fn set_bandwidth_limit(mut self, bytes_per_seconds: Option<usize>, drop_ratio: f64) -> Self {
        match bytes_per_seconds {
            Some(bytes_per_seconds) if bytes_per_seconds > 0 => {
                self.bandwidth_limit = Some(Some(BandwithLimit {
                    window: SlidingWindowCounter::new_with_time_provider(
                        bytes_per_seconds as _,
                        1000, /* ms */
                        || SystemTime::now().duration_since(UNIX_EPOCH).unwrap(),
                    ),
                    drop_ratio,
                }));
            }
            _ => {
                self.bandwidth_limit = Some(None);
            }
        }
        self
    }

    /// Set the latency distribution function. It produces an optional [`Duration`] that represents the latency to be
    /// added to the packet. If the function returns `None`, no latency will be added.
    pub fn set_latency_distribution<F>(mut self, f: Option<F>) -> Self
    where
        F: FnMut() -> Option<Duration> + Send + Sync + 'static,
    {
        if let Some(f) = f {
            self.latency_distribution = Some(Some(Box::new(f)));
        } else {
            self.latency_distribution = Some(None);
        }
        self
    }

    /// Set the probability of packet drop (0.0 to 1.0).
    pub fn set_drop_probability(mut self, probability: Option<f64>) -> Self {
        self.drop_probability = probability;
        self
    }

    /// Set the probability of packet corruption (0.0 to 1.0).
    pub fn set_corrupt_probability(mut self, probability: Option<f64>) -> Self {
        self.corrupt_probability = probability;
        self
    }

    /// Set the probability of packet duplication (0.0 to 1.0).
    pub fn set_duplicate_probability(mut self, probability: Option<f64>) -> Self {
        self.duplicate_probability = probability;
        self
    }

    /// Change the item ordering behavior. See [`ChokeSettingsOrder`] for more information.
    pub fn set_ordering(mut self, ordering: Option<ChokeSettingsOrder>) -> Self {
        self.ordering = ordering;
        self
    }
}
