#[derive(Clone, Default)]
pub struct TimeSelection {
    pub start_track: usize,
    pub end_track: usize,
    pub start_beats: f64,
    pub end_beats: f64,
}

impl TimeSelection {
    pub fn normalized(&self) -> (std::ops::RangeInclusive<usize>, std::ops::Range<f64>) {
        let track_range = self.start_track.min(self.end_track)..=self.start_track.max(self.end_track);
        let time_range = self.start_beats.min(self.end_beats)..self.start_beats.max(self.end_beats);
        (track_range, time_range)
    }
}