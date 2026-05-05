use std::fmt::Debug;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{fs::File, io, path::Path, range::Range, sync::Arc};

use itertools::{Itertools, MinMaxResult};
use symphonia::{
    core::{
        audio::{Channels, Signal},
        codecs::{Decoder, DecoderOptions},
        errors::{Error as SymphoniaError, Result as SymphoniaResult},
        formats::{SeekMode, SeekTo},
    },
    default::get_codecs,
};
use tap::Pipe;
use tracing::{error, instrument};

use crate::{
    processing::time::{Beats, Samples, Tempo, Time},
    read::Reader,
};

pub static NEXT_CLIP_ID: AtomicUsize = AtomicUsize::new(1);

#[derive(Clone, Debug)]
pub enum ClipData {
    Audio(AudioClipData),
    Symphonia(SymphoniaClipData),
}

#[derive(Clone, Debug)]
pub struct AudioClipData {
    /// Interleaved audio data; a flattened array of frames.
    pub(crate) data: Arc<[f32]>,
    pub channels: usize,
}

pub struct SymphoniaClipData {
    pub(crate) path: Arc<Path>,
    /// The track index within the file.
    pub(crate) track: usize,
    pub(crate) reader: Reader,
    pub(crate) decoder: Box<dyn Decoder>,
}

impl Debug for SymphoniaClipData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SymphoniaClipData").field("path", &self.path).field("track", &self.track).finish_non_exhaustive()
    }
}

impl Clone for SymphoniaClipData {
    /// Cloning reopens the file and recreates a new decoder from the same track.
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
    /// Return a new clip for each track in the file at the given path.
    ///
    /// # Errors
    /// If there was a problem opening the file while trying to count the tracks, return an error.
    /// Each clip in the iterator may also be an error if there was a problem opening the file or creating the decoder for that track.
    pub fn from_path(path: Arc<Path>) -> SymphoniaResult<impl ExactSizeIterator<Item = SymphoniaResult<Self>>> {
        let reader = {
            let path = Arc::clone(&path);
            move || Reader::new(File::open(&path).map_err(SymphoniaError::IoError)?)
        };
        let len = reader()?.format_reader.tracks().len();
        Ok((0..len).map(move |index| {
            let reader = reader()?;
            Ok(Self {
                path: Arc::clone(&path),
                track: index,
                decoder: get_codecs().make(&reader.format_reader.tracks()[index].codec_params, &DecoderOptions::default())?,
                reader,
            })
        }))
    }
}

#[derive(Clone, Debug)]
pub struct Clip {
    pub id: usize,
    pub name: String,
    pub(crate) data: ClipData,
    pub(crate) is_stretched: bool,
    pub(crate) stretched_samples: Arc<[f32]>,
    pub timing: ClipTiming,
}

impl Clip {
    pub fn new(name: String, data: ClipData, is_stretched: bool, stretched_samples: Arc<[f32]>, timing: ClipTiming) -> Self {
        Self {
            id: NEXT_CLIP_ID.fetch_add(1, Ordering::Relaxed),
            name,
            data,
            is_stretched,
            stretched_samples,
            timing,
        }
    }

    #[must_use]
    pub fn clone_with_new_id(&self) -> Self {
        Self {
            id: NEXT_CLIP_ID.fetch_add(1, Ordering::Relaxed),
            name: self.name.clone(),
            data: self.data.clone(),
            is_stretched: false,
            stretched_samples: Arc::new([]),
            timing: self.timing,
        }
    }

    #[must_use]
    #[allow(clippy::cast_precision_loss, reason = "frame count is unlikely to be that large")]
    pub fn data_len(&self) -> Time {
        match &self.data {
            ClipData::Audio(AudioClipData { data, channels }) => Time::Samples(Samples((data.len() / channels) as f64)),
            ClipData::Symphonia(SymphoniaClipData { decoder, .. }) => Time::Samples(Samples(decoder.codec_params().n_frames.unwrap_or(0) as f64)),
        }
    }

    /// Generate a mipmap of the clip's data at the given resolution in samples.
    /// Each element represents one channel which is a vector of min-max ranges for each chunk of the clip.
    ///
    /// # Errors
    /// Returns an error if there was a problem reading or decoding the clip's data.
    #[instrument]
    pub fn base_minmax_mipmap(&mut self, tempo: Tempo, samples_per_chunk: usize) -> SymphoniaResult<Vec<Vec<Range<f32>>>> {
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
                (0..channels)
                    .map(|c| {
                        (0..chunks)
                            .map(|x| {
                                let start_frame = x * samples_per_chunk;
                                let end_frame = start_frame + samples_per_chunk;
                                from_minmax(
                                    (start_frame..end_frame)
                                        .map(|frame_index| frame_index * channels + c)
                                        .take_while(|sample_index| sample_index < &data.len())
                                        .map(|sample_index| data[sample_index])
                                        .minmax(),
                                    0.,
                                )
                            })
                            .collect()
                    })
                    .collect_vec()
                    .pipe(Ok)
            }
            ClipData::Symphonia(SymphoniaClipData { track, reader, decoder, .. }) => {
                reader.format_reader.seek(
                    SeekMode::Accurate,
                    SeekTo::TimeStamp {
                        ts: 0,
                        track_id: reader.format_reader.tracks()[*track].id,
                    },
                )?;
                let mut channels = vec![
                    Vec::new();
                    decoder.codec_params().channels.map_or_else(
                        || {
                            error!("no channel data for track");
                            1
                        },
                        Channels::count
                    )
                ];
                loop {
                    let packet = match reader.format_reader.next_packet() {
                        Ok(packet) => packet,
                        Err(SymphoniaError::IoError(error)) if error.kind() == io::ErrorKind::UnexpectedEof => break,
                        Err(error) => return Err(error),
                    };
                    if packet.track_id() != reader.format_reader.tracks()[*track].id {
                        continue;
                    }
                    let Ok(source) = decoder.decode(&packet) else { continue };
                    let mut destination = source.make_equivalent::<f32>();
                    source.convert(&mut destination);

                    for (index, channel) in channels.iter_mut().enumerate() {
                        channel.extend(destination.chan(index).iter().copied());
                    }
                }
                channels
                    .into_iter()
                    .map(|data| {
                        (0..chunks)
                            .map(|x| {
                                let start = x * samples_per_chunk;
                                let end = start + samples_per_chunk;
                                let range = start.min(data.len().saturating_sub(1))..end.min(data.len().saturating_sub(1));
                                if range.is_empty() {
                                    let default = *data.get(range.start).unwrap_or(&0.0);
                                    from_minmax(MinMaxResult::NoElements, default)
                                } else {
                                    from_minmax(data[range].iter().copied().minmax(), 0.0)
                                }
                            })
                            .collect()
                    })
                    .collect_vec()
                    .pipe(Ok)
            }
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum ClipTiming {
    Beats(ClipTimingBeats),
    Samples(ClipTimingSamples),
}

#[derive(Copy, Clone, Debug)]
pub struct ClipTimingBeats {
    pub start: Beats,
    pub end: Beats,
    pub offset: Beats,
}

#[derive(Copy, Clone, Debug)]
pub struct ClipTimingSamples {
    pub start: Samples,
    pub end: Samples,
    pub offset: Samples,
}

impl ClipTimingBeats {
    #[must_use]
    pub fn as_samples(self, tempo: Tempo) -> ClipTimingSamples {
        ClipTimingSamples {
            start: self.start.samples(tempo),
            end: self.end.samples(tempo),
            offset: self.offset.samples(tempo),
        }
    }
    #[must_use]
    pub const fn downgrade(self) -> ClipTiming {
        ClipTiming::Beats(self)
    }
    #[must_use]
    pub fn len(self) -> Beats {
        Beats::new(self.end.0 - self.start.0)
    }
}

impl ClipTimingSamples {
    #[must_use]
    pub fn as_beats(self, tempo: Tempo) -> ClipTimingBeats {
        ClipTimingBeats {
            start: self.start.beats(tempo),
            end: self.end.beats(tempo),
            offset: self.offset.beats(tempo),
        }
    }
    #[must_use]
    pub const fn downgrade(self) -> ClipTiming {
        ClipTiming::Samples(self)
    }
    #[must_use]
    pub fn len(self) -> Samples {
        Samples::new(self.end.0 - self.start.0)
    }
}

impl ClipTiming {
    #[must_use]
    pub fn as_beats(self, tempo: Tempo) -> ClipTimingBeats {
        match self {
            Self::Beats(beats) => beats,
            Self::Samples(samples) => samples.as_beats(tempo),
        }
    }

    #[must_use]
    pub fn as_samples(self, tempo: Tempo) -> ClipTimingSamples {
        match self {
            Self::Beats(beats) => beats.as_samples(tempo),
            Self::Samples(samples) => samples,
        }
    }

    #[must_use]
    pub fn overlaps(self, other: Self, tempo: Tempo) -> bool {
        let a = self.as_samples(tempo);
        let b = other.as_samples(tempo);
        a.start < b.end && a.end > b.start
    }
}
