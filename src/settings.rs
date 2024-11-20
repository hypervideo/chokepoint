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
    pub(crate) settings_rx: Option<mpsc::Receiver<ChokeSettings>>,
}

impl ChokeSettings {
    pub fn settings_updater(&mut self) -> mpsc::Sender<ChokeSettings> {
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
