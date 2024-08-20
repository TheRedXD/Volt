use std::{cmp::Ordering, collections::BTreeSet, path::PathBuf};
use strum::Display;

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

pub struct Browser {
    pub entries: BTreeSet<BrowserEntry>,
    pub selected_category: BrowserCategory,
    pub path: PathBuf,
}