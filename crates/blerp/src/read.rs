use symphonia::{
    core::{
        errors::Result as SymphoniaResult,
        formats::{FormatOptions, FormatReader},
        io::{MediaSource, MediaSourceStream, MediaSourceStreamOptions},
        meta::MetadataOptions,
        probe::Hint,
    },
    default::get_probe,
};

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
}
