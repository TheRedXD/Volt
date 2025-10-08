use std::{cell::RefCell, num::NonZeroUsize, rc::Rc, time::Duration};

use itertools::Itertools;
use symphonia::{
    core::{
        audio::Channels,
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
