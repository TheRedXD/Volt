pub mod export;
pub mod generation;
pub mod live;

pub fn effect_clipper(threshold: f64, sample: f64) -> f64 {
    if sample > threshold {
        threshold
    } else if sample < -threshold {
        -threshold
    } else {
        sample
    }
}

pub fn effect_volume(volume: f64, sample: f64) -> f64 {
    sample * volume
}
