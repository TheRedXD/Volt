use std::{fs::File, io, path::Path, range::Range, sync::Arc};
use std::sync::atomic::{AtomicUsize, Ordering};

use itertools::{Itertools, MinMaxResult};
use symphonia::{
    core::{
        audio::Signal,
        codecs::{Decoder, DecoderOptions},
        errors::Error as SymphoniaError,
        formats::{SeekMode, SeekTo},
        units::Time as SymphoniaTime,
    },
    default::get_codecs,
};

use crate::{
    SAMPLE_RATE,
    processing::time::{Beats, Samples, Tempo, Time},
    read::Reader,
};

pub(crate) static NEXT_CLIP_ID: AtomicUsize = AtomicUsize::new(1);

#[derive(Clone)]
pub enum ClipData {
    Audio(AudioClipData),
    Symphonia(SymphoniaClipData),
}

#[derive(Clone)]
pub struct AudioClipData {
    pub(crate) data: Arc<[f32]>,
    pub channels: usize,
}

pub struct SymphoniaClipData {
    pub(crate) path: Arc<Path>,
    pub(crate) track: usize,
    pub(crate) reader: Reader,
    pub(crate) decoder: Box<dyn Decoder>,
}

impl Clone for SymphoniaClipData {
    fn clone(&self) -> Self {
        let reader = Reader::new(File::open(&self.path).unwrap()).unwrap();
        Self {
            path: Arc::clone(&self.path),
            track: self.track,
            decoder: get_codecs().make(&reader.format_reader.tracks()[self.track].codec_params, &DecoderOptions::default()).unwrap(),
            reader,
        }
    }
}

impl SymphoniaClipData {
    pub fn from_path(path: Arc<Path>) -> impl ExactSizeIterator<Item = Self> {
        let reader = {
            let path = Arc::clone(&path);
            move || Reader::new(File::open(&path).unwrap()).unwrap()
        };
        let len = reader().format_reader.tracks().len();
        (0..len).map(move |index| {
            let reader = reader();
            Self {
                path: Arc::clone(&path),
                track: index,
                decoder: get_codecs().make(&reader.format_reader.tracks()[index].codec_params, &DecoderOptions::default()).unwrap(),
                reader,
            }
        })
    }
}

#[derive(Clone)]
pub struct Clip {
    pub id: usize,
    pub name: String,
    pub(crate) data: ClipData,
    pub timing: ClipTiming,
}

impl Clip {
    pub fn new(name: String, data: ClipData, timing: ClipTiming) -> Self {
        Self {
            id: NEXT_CLIP_ID.fetch_add(1, Ordering::Relaxed),
            name,
            data,
            timing,
        }
    }

    pub fn clone_with_new_id(&self) -> Self {
        Self {
            id: NEXT_CLIP_ID.fetch_add(1, Ordering::Relaxed),
            name: self.name.clone(),
            data: self.data.clone(),
            timing: self.timing,
        }
    }

    pub fn data_len(&self) -> Time {
        match &self.data {
            ClipData::Audio(AudioClipData { data, channels }) => Time::Samples(Samples((data.len() / channels) as f64)),
            ClipData::Symphonia(SymphoniaClipData { decoder, .. }) => Time::Samples(Samples(decoder.codec_params().n_frames.unwrap_or(0) as f64)),
        }
    }

    pub fn base_minmax_mipmap(&mut self, tempo: Tempo, samples_per_chunk: usize) -> Vec<Vec<Range<f32>>> {
        let chunks = self.data_len().samples(tempo).usize() / samples_per_chunk;
        let from_minmax = |minmax, default| {
            Range::from(match minmax {
                MinMaxResult::NoElements => default..default,
                MinMaxResult::OneElement(sample) => sample..sample,
                MinMaxResult::MinMax(min, max) => min..max,
            })
        };

        match &mut self.data {
            ClipData::Audio(AudioClipData { data, channels }) => {
                let channels = *channels;
                (0..channels).map(|c| {
                    (0..chunks)
                        .map(|x| {
                            let start_frame = x * samples_per_chunk;
                            let end_frame = start_frame + samples_per_chunk;
                            let mut min = f32::MAX;
                            let mut max = f32::MIN;
                            let mut count = 0;
                            for frame in start_frame..end_frame {
                                let idx = frame * channels + c;
                                if idx < data.len() {
                                    let val = data[idx];
                                    if val < min { min = val; }
                                    if val > max { max = val; }
                                    count += 1;
                                }
                            }
                            if count == 0 {
                                let def_idx = start_frame * channels + c;
                                let default = if def_idx < data.len() { data[def_idx] } else { 0.0 };
                                from_minmax(MinMaxResult::NoElements, default)
                            } else {
                                from_minmax(MinMaxResult::MinMax(min, max), 0.0)
                            }
                        })
                        .collect()
                }).collect()
            },
            ClipData::Symphonia(SymphoniaClipData { track, reader, decoder, .. }) => {
                reader
                    .format_reader
                    .seek(
                        SeekMode::Accurate,
                        SeekTo::TimeStamp {
                            ts: 0,
                            track_id: reader.format_reader.tracks()[*track].id,
                        },
                    )
                    .unwrap();
                let mut channel_data = vec![Vec::new(); decoder.codec_params().channels.map_or(1, |c| c.count())];
                loop {
                    let packet = match reader.format_reader.next_packet() {
                        Ok(packet) => packet,
                        Err(SymphoniaError::IoError(error)) if error.kind() == io::ErrorKind::UnexpectedEof => break,
                        Err(error) => panic!("{}", error)
                    };
                    if packet.track_id() != reader.format_reader.tracks()[*track].id {
                        continue;
                    }
                    let source = match decoder.decode(&packet) {
                        Ok(audio) => audio,
                        Err(_) => continue,
                    };
                    let mut destination = source.make_equivalent::<f32>();
                    source.convert(&mut destination);
                    for c in 0..destination.spec().channels.count() {
                        if c < channel_data.len() {
                            channel_data[c].extend(destination.chan(c).iter().copied());
                        }
                    }
                }
                channel_data.into_iter().map(|data| {
                    (0..chunks)
                        .map(|x| {
                            let start = x * samples_per_chunk;
                            let end = start + samples_per_chunk;
                            let range = start.min(data.len().saturating_sub(1))..end.min(data.len().saturating_sub(1));
                            if range.is_empty() {
                                let default = *data.get(range.start).unwrap_or(&0.0);
                                from_minmax(MinMaxResult::NoElements, default)
                            } else {
                                from_minmax(data[range.clone()].iter().copied().minmax(), 0.0)
                            }
                        })
                        .collect()
                }).collect()
            }
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
        Beats::new(self.end.0 - self.start.0)
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
        Samples::new(self.end.0 - self.start.0)
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