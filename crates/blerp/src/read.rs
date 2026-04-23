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
        audio::{AudioBuffer, Channels},
        codecs::{CodecParameters, Decoder, DecoderOptions},
        errors::{Error as SymphoniaError, Result as SymphoniaResult},
        formats::{FormatOptions, FormatReader, Track},
        io::{MediaSource, MediaSourceStream, MediaSourceStreamOptions},
        meta::MetadataOptions,
        probe::Hint,
        units::Time,
    },
    default::{get_codecs, get_probe},
};
use tap::{Pipe, Tap};

pub struct Reader {
    pub format_reader: Box<dyn FormatReader>,
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
        Ok(Self { format_reader })
    }

    pub fn read(&mut self, decoder: &mut dyn Decoder) -> SymphoniaResult<AudioBuffer<f64>> {
        let packet = self.format_reader.next_packet().unwrap();
        let source = decoder.decode(&packet).unwrap();
        let mut destination = source.make_equivalent::<f64>();
        source.convert(&mut destination);
        Ok(destination)
    }
}
