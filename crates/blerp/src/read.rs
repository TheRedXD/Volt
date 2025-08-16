use itertools::Itertools;
use symphonia::{
    core::{
        audio::AudioBuffer,
        codecs::DecoderOptions,
        conv::FromSample,
        errors::Result as SymphoniaResult,
        formats::FormatOptions,
        io::{MediaSource, MediaSourceStream, MediaSourceStreamOptions},
        meta::MetadataOptions,
        probe::Hint,
        sample::{Sample, i24, u24},
    },
    default::{get_codecs, get_probe},
};
use tap::Pipe;

#[derive(Clone, Debug)]
pub struct Track<S> {
    pub channels: Vec<Channel<S>>,
    pub sample_rate: Option<u32>,
}

#[derive(Clone)]
pub struct Channel<S> {
    pub samples: Vec<S>,
}

impl<S> std::fmt::Debug for Channel<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Channel {{ samples: vec![...; {}] }}", self.samples.len())
    }
}

/// Read audio samples from some media source and return an iterator over tracks in the source.
///
/// # Errors
///
/// Returns an error if any of:
/// - the media source format is invalid ([`symphonia::core::probe::Probe::format`] failed)
/// - there are no decoders that supports the codec or there are invalid codec parameters ([`symphonia::core::codecs::CodecRegistry::make`] failed)
/// - an undecodeable packet is encountered ([`symphonia::core::codecs::Decoder::decode`] failed)
pub fn read<
    S: Sample + FromSample<u8> + FromSample<u16> + FromSample<u24> + FromSample<u32> + FromSample<i8> + FromSample<i16> + FromSample<i24> + FromSample<i32> + FromSample<f32> + FromSample<f64>,
    M: MediaSource + 'static,
>(
    source: impl Into<Box<M>>,
) -> SymphoniaResult<impl Iterator<Item = SymphoniaResult<Track<S>>>> {
    let mut format_reader = get_probe()
        .format(
            &Hint::new(),
            MediaSourceStream::new(source.into(), MediaSourceStreamOptions::default()),
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )?
        .format;
    format_reader
        .tracks()
        .iter()
        .map(|track| track.codec_params.clone())
        .collect_vec()
        .into_iter()
        .map(move |codec_params| {
            let mut decoder = get_codecs().make(&codec_params, &DecoderOptions::default())?;
            let mut track = Vec::new();
            while let Ok(packet) = format_reader.next_packet() {
                let r#in = decoder.decode(&packet)?;
                let mut out = AudioBuffer::new(r#in.capacity() as u64, *r#in.spec());
                r#in.convert(&mut out);
                if track.is_empty() {
                    for _ in 0..out.planes().planes().len() {
                        track.push(Channel { samples: Vec::new() });
                    }
                }
                for (channel, track_channel) in out.planes().planes().iter().zip(track.iter_mut()) {
                    track_channel.samples.extend_from_slice(channel);
                }
            }
            Ok(Track {
                channels: track,
                sample_rate: codec_params.sample_rate,
            })
        })
        .pipe(Ok)
}
