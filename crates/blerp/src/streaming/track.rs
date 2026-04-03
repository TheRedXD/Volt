use crate::streaming::clip::Clip;


pub struct Track {
    pub(crate) clips: Vec<Clip>,
}

impl Track {
    pub fn new() -> Self {
        Self { clips: Vec::new() }
    }

    pub fn clips(&self) -> &[Clip] {
        &self.clips
    }
}
