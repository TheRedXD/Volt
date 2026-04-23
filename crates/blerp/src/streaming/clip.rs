use std::{fs::File, io, path::Path, range::Range, sync::Arc};

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

#[derive(Clone)]
pub enum ClipData {
    Audio(AudioClipData),
    Symphonia(SymphoniaClipData),
}

#[derive(Clone)]
pub struct AudioClipData {
    pub(crate) data: Arc<[f32]>,
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
        (0..reader().format_reader.tracks().len()).map(move |index| {
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
    pub(crate) data: ClipData,
    pub timing: ClipTiming,
}

impl Clip {
    pub fn data_len(&self) -> Time {
        match &self.data {
            ClipData::Audio(AudioClipData { data }) => Time::Samples(Samples(data.len() as f64)),
            ClipData::Symphonia(SymphoniaClipData { decoder, .. }) => Time::Samples(Samples(decoder.codec_params().n_frames.unwrap() as f64)),
        }
    }

    pub fn base_minmax_mipmap(&mut self, tempo: Tempo, samples_per_chunk: usize) -> Vec<Range<f32>> {
        let chunks = self.data_len().samples(tempo).usize() / samples_per_chunk;
        let from_minmax = |minmax, default| {
            Range::from(match minmax {
                MinMaxResult::NoElements => default..default,
                MinMaxResult::OneElement(sample) => sample..sample,
                MinMaxResult::MinMax(min, max) => min..max,
            })
        };
        let from_samples = |data: &[f32]| {
            (0..chunks)
                .map(|x| {
                    let x = x as f64;
                    let start = x * samples_per_chunk as f64;
                    let end = start + samples_per_chunk as f64;
                    let range = Range::from((start as usize).min(data.len() - 1)..(end as usize).min(data.len() - 1));
                    from_minmax(data[range].iter().copied().minmax(), data[range.start])
                })
                .collect()
        };
        match &mut self.data {
            ClipData::Audio(AudioClipData { data }) => from_samples(data),
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
                let mut data = Vec::<f32>::with_capacity(decoder.codec_params().n_frames.unwrap() as usize);
                loop {
                    let packet = match reader.format_reader.next_packet() {
                        Ok(packet) => packet,
                        Err(SymphoniaError::IoError(error)) if error.kind() == io::ErrorKind::UnexpectedEof => break,
                        Err(error) => {
                            panic!("{}", error);
                        }
                    };
                    let source = decoder.decode(&packet).unwrap();
                    let mut destination = source.make_equivalent::<f32>();
                    source.convert(&mut destination);
                    data.extend(destination.chan(0).iter().copied());
                }
                from_samples(&data)
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
