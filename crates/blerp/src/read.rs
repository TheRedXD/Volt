use std::{
    cell::RefCell,
    iter::{IntoIterator, Iterator, from_fn},
    num::NonZeroUsize,
    rc::Rc,
    time::Duration,
};

use itertools::Itertools;
use symphonia::{
    core::{
        audio::{AudioBuffer, Channels, SampleBuffer},
        codecs::{CodecParameters, Decoder, DecoderOptions},
        errors::Result as SymphoniaResult,
        formats::{FormatOptions, FormatReader},
        io::{MediaSource, MediaSourceStream, MediaSourceStreamOptions},
        meta::MetadataOptions,
        probe::Hint,
    },
    default::{get_codecs, get_probe},
};
use tap::Pipe;

pub struct Reader {
    format_reader: Box<dyn FormatReader>,
    track_decoders: Vec<Box<dyn Decoder>>,
}

impl Reader {
    /// Parse the given media source (see [`MediaSource`]) and return a handle which can lazily read it (to avoid loading the whole file into memory).
    ///
    /// # Errors
    ///
    /// Returns an error if any of:
    /// - the media source format is invalid ([`symphonia::core::probe::Probe::format`] failed)
    /// - there are no decoders that supports the codec or there are invalid codec parameters ([`symphonia::core::codecs::CodecRegistry::make`] failed)
    pub fn new<M: MediaSource + 'static>(source: impl Into<Box<M>>) -> SymphoniaResult<Self> {
        let format_reader = get_probe()
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
            .map(move |track| get_codecs().make(&track.codec_params, &DecoderOptions::default()))
            .try_collect::<_, Vec<_>, _>()?
            .pipe(|track_decoders| Self { format_reader, track_decoders })
            .pipe(Ok)
    }

    // TODO make this handle durations of tracks with different start points
    #[must_use]
    pub fn duration(&self) -> Duration {
        self.track_decoders.iter().map(|decoder| decoder_duration(&**decoder)).max().unwrap_or_default()
    }

    pub fn decompose(self) -> impl Iterator<Item = TrackReader> {
        let format_reader = Rc::new(RefCell::new(self.format_reader));
        self.track_decoders.into_iter().map(move |decoder| TrackReader {
            decoder,
            format_reader: Rc::clone(&format_reader),
        })
    }

    /// Yield one packet from this reader as channel-interleaved samples.
    ///
    /// # Errors
    ///
    /// Returns an error if a packet could not be retrieved ([`FormatReader::next_packet`] failed)
    /// or a packet could not be decoded ([`Decoder::decode`] failed).
    // TODO make this handle tracks with different start points
    pub fn packet(&mut self) -> SymphoniaResult<impl Iterator<Item = f32>> {
        let packet = self.format_reader.next_packet()?;
        let mut output = Vec::new();
        for decoder in &mut self.track_decoders {
            let source = decoder.decode(&packet)?;
            let mut destination = AudioBuffer::new(source.capacity() as u64, *source.spec());
            source.convert(&mut destination);
            output.resize_with(output.len().max(destination.spec().channels.count()), Vec::new);
            for (output_channel, packet_channel) in output.iter_mut().zip(destination.planes().planes().iter()) {
                output_channel.resize(packet_channel.len(), 0.);
                for (output_sample, packet_sample) in output_channel.iter_mut().zip(packet_channel.iter()) {
                    *output_sample += packet_sample;
                }
            }
        }

        let mut channels = output.into_iter().map(IntoIterator::into_iter).collect_vec();
        from_fn(move || channels.iter_mut().map(Iterator::next).collect::<Option<Vec<_>>>()).flatten().pipe(Ok)
    }

    #[must_use]
    pub fn sample_rate(&self) -> Option<u32> {
        self.track_decoders.iter().filter_map(|decoder| decoder.codec_params().sample_rate).unique().exactly_one().ok()
    }
}

pub struct TrackReader {
    decoder: Box<dyn Decoder>,
    format_reader: Rc<RefCell<Box<dyn FormatReader>>>,
}

impl TrackReader {
    #[must_use]
    pub fn duration(&self) -> Duration {
        decoder_duration(&*self.decoder)
    }

    pub fn channels(&self) -> NonZeroUsize {
        self.decoder.codec_params().channels.map(Channels::count).and_then(NonZeroUsize::new).unwrap_or(NonZeroUsize::MIN)
    }
}

fn decoder_duration(decoder: &(impl Decoder + ?Sized)) -> Duration {
    let &CodecParameters {
        sample_rate: Some(sample_rate),
        n_frames: Some(n_frames),
        ..
    } = decoder.codec_params()
    else {
        todo!("handle this case")
    };
    #[allow(clippy::cast_precision_loss, reason = "n_frames is not that big and precision of the duration is not critical anyways")]
    Duration::from_secs_f64(n_frames as f64 / f64::from(sample_rate))
}
