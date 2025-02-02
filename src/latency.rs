use rand_distr::{
    Distribution as _,
    Normal,
    SkewNormal,
};
use std::time::Duration;

/// Uses [`rand_distr::Normal`] to generate a normal distribution.
pub fn normal_distribution(
    mean: f64,
    std_dev: f64,
    max: f64,
) -> Option<impl FnMut() -> Option<Duration> + Send + Sync + 'static> {
    let normal = Normal::new(mean, std_dev).unwrap(); // mean = 10ms, std dev = 15ms
    Some(move || {
        let latency = normal.sample(&mut rand::rng()).clamp(0.0, max) as u64;
        (latency > 0).then(|| std::time::Duration::from_millis(latency))
    })
}

/// Uses [`rand_distr::SkewNormal`] to generate a skewed distribution.
pub fn skewed_distribution(
    location: f64,
    scale: f64,
    shape: f64,
    max: f64,
) -> Option<impl FnMut() -> Option<Duration> + Send + Sync + 'static> {
    let skew_normal = SkewNormal::new(location, scale, shape).unwrap(); // location = 10ms, scale = 15ms, shape = 0.5
    Some(move || {
        let latency = skew_normal.sample(&mut rand::rng()).clamp(0.0, max) as u64;
        (latency > 0).then(|| std::time::Duration::from_millis(latency))
    })
}
