use std::ops::{Add, AddAssign, Sub};

use crate::SAMPLE_RATE;

#[derive(Debug, Clone, Copy)]
pub struct Tempo {
    beats_per_hectominute: u32,
}

impl Default for Tempo {
    fn default() -> Self {
        Self::from_bpm(120.)
    }
}

impl Tempo {
    pub fn from_bpm(bpm: f64) -> Self {
        #[allow(clippy::cast_sign_loss, reason = "bpm is always positive")]
        #[allow(clippy::cast_possible_truncation, reason = "bpm only goes up to 999.99, so never truncates")]
        let beats_per_hectominute = (bpm as u32 * 100).clamp(1, 99999);
        Self { beats_per_hectominute }
    }

    pub fn bpm(self) -> f64 {
        f64::from(self.beats_per_hectominute) / 100.
    }

    pub fn bps(self) -> f64 {
        self.bpm() / 60.
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Beats(pub(crate) f64);

impl Beats {
    /// Create a `Beats` from a number of beats.
    ///
    /// # Panics
    ///
    /// Panics if `beats` is negative.
    #[must_use]
    pub fn new(beats: f64) -> Self {
        assert!(beats >= 0., "`Beats` cannot be negative");
        Self(beats)
    }

    #[must_use]
    pub const fn beats(self) -> f64 {
        self.0
    }

    pub fn samples(self, tempo: Tempo) -> Samples {
        Samples::new(self.0 / tempo.bps() * SAMPLE_RATE)
    }
}

impl Sub for Beats {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self((self.0 - rhs.0).max(0.))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TimeSignature {
    pub beats_per_measure: u32,
    pub beat_unit: u32,
}

impl Default for TimeSignature {
    fn default() -> Self {
        Self { beats_per_measure: 4, beat_unit: 4 }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Samples(pub(crate) f64);

impl PartialEq for Samples {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for Samples {}

impl PartialOrd for Samples {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        (self.0).partial_cmp(&other.0)
    }
}

impl Ord for Samples {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl Samples {
    pub const fn new(samples: f64) -> Self {
        assert!(samples >= 0., "`Samples` cannot be negative");
        Self { 0: samples }
    }

    pub fn f64(self) -> f64 {
        self.0
    }

    pub fn u64(self) -> u64 {
        self.0 as u64
    }

    pub fn usize(self) -> usize {
        self.0 as usize
    }

    pub fn beats(self, tempo: Tempo) -> Beats {
        Beats::new(self.0 / SAMPLE_RATE as f64 * tempo.bps())
    }
}

impl Sub for Samples {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self((self.0 - rhs.0).max(0.))
    }
}

impl Add for Samples {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl AddAssign for Samples {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0
    }
}

pub enum Time {
    Beats(Beats),
    Samples(Samples),
}

impl Time {
    pub fn beats(self, tempo: Tempo) -> Beats {
        match self {
            Self::Beats(beats) => beats,
            Self::Samples(samples) => samples.beats(tempo),
        }
    }

    pub fn samples(self, tempo: Tempo) -> Samples {
        match self {
            Self::Beats(beats) => beats.samples(tempo),
            Self::Samples(samples) => samples,
        }
    }
}
