use std::sync::Arc;

use crate::processing::time::{Beats, Samples, Tempo, Time};

pub enum ClipData {
    Audio(AudioClipData),
}

pub struct AudioClipData {
    pub(crate) data: Arc<[f32]>,
}

pub struct Clip {
    pub(crate) data: ClipData,
    pub timing: ClipTiming,
}

impl Clip {
    pub fn data_len(&self) -> Time {
        match &self.data {
            ClipData::Audio(AudioClipData { data }) => Time::Samples(Samples(data.len() as f64)),
        }
    }
}

#[derive(Copy, Clone)]
pub enum ClipTiming {
    Beats(ClipTimingBeats),
    Samples(ClipTimingSamples),
}

#[derive(Copy, Clone)]
pub struct ClipTimingBeats {
    pub start: Beats,
    pub end: Beats,
    pub offset: Beats,
}

#[derive(Copy, Clone)]
pub struct ClipTimingSamples {
    pub start: Samples,
    pub end: Samples,
    pub offset: Samples,
}

impl ClipTimingBeats {
    pub fn as_samples(self, tempo: Tempo) -> ClipTimingSamples {
        ClipTimingSamples {
            start: self.start.samples(tempo),
            end: self.end.samples(tempo),
            offset: self.offset.samples(tempo),
        }
    }
    pub fn downgrade(self) -> ClipTiming {
        ClipTiming::Beats(self)
    }
    pub fn len(self) -> Beats {
        self.end - self.start
    }
}

impl ClipTimingSamples {
    pub fn as_beats(self, tempo: Tempo) -> ClipTimingBeats {
        ClipTimingBeats {
            start: self.start.beats(tempo),
            end: self.end.beats(tempo),
            offset: self.offset.beats(tempo),
        }
    }
    pub fn downgrade(self) -> ClipTiming {
        ClipTiming::Samples(self)
    }
    pub fn len(self) -> Samples {
        self.end - self.start
    }
}

impl ClipTiming {
    pub fn as_beats(self, tempo: Tempo) -> ClipTimingBeats {
        match self {
            Self::Beats(beats) => beats,
            Self::Samples(samples) => samples.as_beats(tempo),
        }
    }

    pub fn as_samples(self, tempo: Tempo) -> ClipTimingSamples {
        match self {
            Self::Beats(beats) => beats.as_samples(tempo),
            Self::Samples(samples) => samples,
        }
    }

    pub fn overlaps(self, other: Self, tempo: Tempo) -> bool {
        let a = self.as_samples(tempo);
        let b = other.as_samples(tempo);
        a.start < b.end && a.end > b.start
    }
}
