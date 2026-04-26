use std::{cmp::Ordering, ops::{Add, AddAssign}};

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
    #[must_use]
    pub fn from_bpm(bpm: f64) -> Self {
        #[allow(clippy::cast_sign_loss, reason = "bpm is always positive")]
        #[allow(clippy::cast_possible_truncation, reason = "bpm only goes up to 999.99, so never truncates")]
        let beats_per_hectominute = ((bpm * 100.) as u32).clamp(1, 99999);
        Self { beats_per_hectominute }
    }

    #[must_use]
    pub fn bpm(self) -> f64 {
        f64::from(self.beats_per_hectominute) / 100.
    }

    #[must_use]
    pub fn bps(self) -> f64 {
        self.bpm() / 60.
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Beats(pub(crate) f64);

impl Beats {
    /// Create a `Beats` from a number of beats.
    #[must_use]
    pub const fn new(beats: f64) -> Self {
        Self(beats)
    }

    #[must_use]
    pub fn from_u32(beats: u32) -> Self {
        Self(f64::from(beats))
    }

    #[must_use]
    pub const fn f64(self) -> f64 {
        self.0
    }

    #[must_use]
    #[allow(clippy::cast_possible_truncation, reason = "beats are unlikely to be that large")]
    pub const fn f32(self) -> f32 {
        self.0 as f32
    }

    #[must_use]
    #[allow(clippy::cast_possible_truncation, reason = "beats are unlikely to be that large")]
    #[allow(clippy::cast_sign_loss, reason = "sign loss is intentional when converting to u32")]
    pub const fn u32(self) -> u32 {
        self.0 as u32
    }

    #[must_use]
    pub fn samples(self, tempo: Tempo) -> Samples {
        Samples::new(self.0 / tempo.bps() * SAMPLE_RATE)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TimeSignature {
    pub beats_per_measure: u32,
    pub beat_value: u32,
}

impl Default for TimeSignature {
    fn default() -> Self {
        Self { beats_per_measure: 4, beat_value: 4 }
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
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Samples {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.partial_cmp(&other.0).unwrap()
    }
}

impl Samples {
    #[must_use]
    pub const fn new(samples: f64) -> Self {
        Self(samples)
    }

    #[must_use]
    pub const fn f64(self) -> f64 {
        self.0
    }

    #[must_use]
    #[allow(clippy::cast_possible_truncation, reason = "samples are unlikely to be that large")]
    #[allow(clippy::cast_sign_loss, reason = "sign loss is intentional when converting to u64")]
    pub const fn u64(self) -> u64 {
        self.0 as u64
    }

    #[must_use]
    #[allow(clippy::cast_possible_truncation, reason = "samples are unlikely to be that large")]
    #[allow(clippy::cast_sign_loss, reason = "sign loss is intentional when converting to usize")]
    pub const fn usize(self) -> usize {
        self.0 as usize
    }

    #[must_use]
    pub fn beats(self, tempo: Tempo) -> Beats {
        Beats::new(self.0 / SAMPLE_RATE * tempo.bps())
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
        self.0 += rhs.0;
    }
}

#[derive(Clone, Copy)]
pub enum Time {
    Beats(Beats),
    Samples(Samples),
}

impl Time {
    #[must_use]
    pub fn beats(self, tempo: Tempo) -> Beats {
        match self {
            Self::Beats(beats) => beats,
            Self::Samples(samples) => samples.beats(tempo),
        }
    }

    #[must_use]
    pub fn samples(self, tempo: Tempo) -> Samples {
        match self {
            Self::Beats(beats) => beats.samples(tempo),
            Self::Samples(samples) => samples,
        }
    }
}
