use egui::ImageSource;
use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use strum::Display;

const MEDIA_EXTENSIONS: [&str; 6] = ["wav", "wave", "mp3", "ogg", "flac", "opus"];

#[derive(Display, Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserCategory {
    Files,
    Devices,
}

#[derive(Display, Debug, Clone, PartialEq, Eq)]
pub enum BrowserEntry {
    Directory(PathBuf),
    Audio(PathBuf),
    File(PathBuf),
}

impl Ord for BrowserEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.path().file_name().cmp(&other.path().file_name())
    }
}

impl PartialOrd for BrowserEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl From<PathBuf> for BrowserEntry {
    fn from(path: PathBuf) -> Self {
        match path.as_path() {
            e if e.is_dir() => BrowserEntry::Directory(path),
            _ if path
                .extension()
                .and_then(|f| f.to_str())
                .is_some_and(|p| MEDIA_EXTENSIONS.contains(&p)) =>
            {
                BrowserEntry::Audio(path)
            }
            _ => BrowserEntry::File(path),
        }
    }
}

impl BrowserEntry {
    pub fn path(&self) -> &Path {
        match self {
            BrowserEntry::Directory(p) => p,
            BrowserEntry::Audio(p) => p,
            BrowserEntry::File(p) => p,
        }
    }

    pub fn is_directory(&self) -> bool {
        matches!(self, BrowserEntry::Directory(_))
    }

    pub fn is_file(&self) -> bool {
        matches!(self, BrowserEntry::File(_))
    }

    pub fn is_audio(&self) -> bool {
        matches!(self, BrowserEntry::Audio(_))
    }

    pub fn image(&self) -> ImageSource {
        match self {
            BrowserEntry::Directory(_) => crate::images::DIRECTORY_IMAGE,
            BrowserEntry::Audio(_) => crate::images::AUDIO_IMAGE,
            BrowserEntry::File(_) => crate::images::FILE_IMAGE,
        }
    }
}
