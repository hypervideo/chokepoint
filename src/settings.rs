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

/// Settings for1
#[derive(Default)]
#[allow(clippy::type_complexity)]
pub struct ChokeSettings {
    pub(crate) latency_distribution: Option<Option<Box<dyn FnMut() -> Option<Duration> + Send + Sync>>>,
    pub(crate) drop_probability: Option<f64>,
    pub(crate) corrupt_probability: Option<f64>,
    pub(crate) duplicate_probability: Option<f64>,
    pub(crate) bandwidth_limiter: Option<Option<SlidingWindowCounter<fn() -> Duration>>>,
    pub(crate) backpressure: Option<bool>,
    pub(crate) settings_rx: Option<mpsc::Receiver<ChokeSettings>>,
}

impl ChokeSettings {
    pub fn with_updater(settings_rx: mpsc::Receiver<ChokeSettings>) -> Self {
        Self {
            settings_rx: Some(settings_rx),
            ..Default::default()
        }
    }

    pub fn settings_updater(&mut self) -> mpsc::Sender<ChokeSettings> {
        let (settings_tx, settings_rx) = mpsc::channel(1);
        self.settings_rx = Some(settings_rx);
        settings_tx
    }

    /// Set the bandwidth limit in bytes per second.
    pub fn set_bandwidth_limit(mut self, bytes_per_seconds: Option<usize>) -> Self {
        if let Some(bytes_per_seconds) = bytes_per_seconds {
            self.bandwidth_limiter = Some(Some(SlidingWindowCounter::new_with_time_provider(
                bytes_per_seconds as _,
                1000, /* ms */
                || SystemTime::now().duration_since(UNIX_EPOCH).unwrap(),
            )));
        } else {
            self.bandwidth_limiter = None;
        }
        self
    }

    /// Set the latency distribution function.
    pub fn set_latency_distribution<F>(mut self, f: Option<F>) -> Self
    where
        F: FnMut() -> Option<Duration> + Send + Sync + 'static,
    {
        if let Some(f) = f {
            self.latency_distribution = Some(Some(Box::new(f)));
        } else {
            self.latency_distribution = None;
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

    /// Enable backpressure. Will not consume from inner stream until the choke queues are empty. Otherwise will consume
    /// from inner stream as fast as possible.
    pub fn set_backpressure(mut self, enable: Option<bool>) -> Self {
        self.backpressure = enable;
        self
    }
}
