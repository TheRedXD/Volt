use std::thread::JoinHandle;
use std::{cmp::Ordering, collections::BTreeSet, fs::File, path::PathBuf, thread::Thread};
use strum::Display;

// FIXME: Temporary rodio playback, might need to use cpal or make rodio proper
use rodio::{Decoder, OutputStream, Sink};
use rodio::source::{SineWave, Source};
use std::time::Duration;

use std::io::BufReader;

#[derive(Display, Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserCategory {
    Files,
    Devices,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserEntry {
    pub path: PathBuf,
    pub kind: BrowserEntryKind,
}

impl Ord for BrowserEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.kind.cmp(&other.kind).then(
            self.path
                .file_name()
                .unwrap()
                .cmp(other.path.file_name().unwrap()),
        )
    }
}

impl PartialOrd for BrowserEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Display, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BrowserEntryKind {
    Directory,
    Audio,
    File,
}

pub struct Preview {
    pub preview_thread: Option<JoinHandle<()>>
}

impl Preview {
    pub fn play_file(&mut self, file: File) {
        // Kill the current thread if it's not sleeping
        if let Some(thread) = self.preview_thread.take() {
            if !thread.is_finished() {
                thread.thread().unpark();
            }
        }

        let file = BufReader::new(file);
        self.preview_thread = Some(std::thread::spawn(move || {
            let (_stream, stream_handle) = OutputStream::try_default().unwrap();
            let source = Decoder::new(file).unwrap();
            let sink = Sink::try_new(&stream_handle).unwrap();
            // let source = SineWave::new(440.0).take_duration(Duration::from_secs_f32(0.25)).amplify(0.20);
            sink.append(source);
            std::thread::park();
        }));
    }
}

pub struct Browser {
    pub entries: BTreeSet<BrowserEntry>,
    pub selected_category: BrowserCategory,
    pub path: PathBuf,
    pub preview: Preview
}