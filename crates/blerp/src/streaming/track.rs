use std::sync::{Arc, Mutex};

use crate::streaming::clip::Clip;

#[derive(Clone)]
pub struct Track {
    pub(crate) clips: Vec<Clip>,
    pub gain: f32
}

impl Track {
    pub fn new() -> Self {
        Self { clips: Vec::new(), gain: 1. }
    }

    pub fn clips(&self) -> &[Clip] {
        &self.clips
    }
}
