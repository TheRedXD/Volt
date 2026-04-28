
use crate::streaming::clip::Clip;

#[derive(Clone)]
pub struct Track {
    pub(crate) clips: Vec<Clip>,
    pub color: u32,
    pub gain: f32
}

impl Default for Track {
    fn default() -> Self {
        Self::new()
    }
}

impl Track {
    #[must_use]
    pub const fn new() -> Self {
        Self { clips: Vec::new(), color: 0x000000, gain: 1. }
    }

    #[must_use]
    pub fn clips(&self) -> &[Clip] {
        &self.clips
    }
}
