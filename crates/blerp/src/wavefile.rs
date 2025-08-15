use std::{
    fmt::{self, Debug},
    hint::unreachable_unchecked,
    io::{self, Write},
    mem::size_of,
    num::NonZeroU16,
};

use cpal::FromSample;
use itertools::Itertools;
use nom::{
    Err,
    combinator::complete,
    error::{ErrorKind, FromExternalError, ParseError},
};
use nom_locate::LocatedSpan;
use num::traits::ToBytes;
use read::{Input, wave_file};
use thiserror::Error;

#[derive(Clone)]
pub struct WaveFile {
    pub format: Format,
    pub channels: NonZeroU16,
    pub sample_rate: u32,
    pub bytes_per_sample: u16,
    pub data: Vec<u8>,
}

impl Debug for WaveFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct Data<'a>(&'a [u8]);
        impl Debug for Data<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "<{} bytes>", self.0.len())
            }
        }
        f.debug_struct("WaveFile")
            .field("format", &self.format)
            .field("channels", &self.channels)
            .field("sample_rate", &self.sample_rate)
            .field("bytes_per_sample", &self.bytes_per_sample)
            .field("data", &Data(&self.data))
            .finish()
    }
}

#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    PulseCodeModulation = 1,
    FloatingPoint = 3,
}

#[derive(Debug)]
pub struct ReadError {
    pub kind: ReadErrorKind,
    pub position: usize,
}

#[derive(Debug)]
pub enum ReadErrorKind {
    /// `nAvgBytesPerSec` is not equal to `nSamplesPerSec * nBlockAlign`.
    DataRateMismatch,
    /// `nChannels` is not equal to `nBlockAlign / wBitsPerSample`.
    ChannelCountMismatch,
    /// `nChannels` was zero.
    NoChannels,
    /// The format is floating point (`wFormatTag == 3`), but the `cbSize` field was not given.
    MissingFormatChunkExtensionSize,
    /// The format was not PCM (`wFormatTag == 1`) or IEEE floating point (`wFormatTag == 3`).
    FormatNotSupported,
    /// The format is floating point (`wFormatTag == 3`), but the `fact` chunk was missing.
    MissingFactChunk,
    /// The `fact` chunk's `dwSampleLength` field was not equal to the number of samples in the data chunk.
    FactChunkLengthMismatch,
    /// The size of the data chunk was not a multiple of the block size.
    DataSizeNotMultipleOfBlockSize,
    /// The size of the data chunk did not match information in the format and `fact` chunks.
    InvalidDataSize,
    /// Other parse error.
    Nom(ErrorKind),
}

impl ParseError<Input<'_>> for ReadError {
    fn from_error_kind(input: Input, kind: ErrorKind) -> Self {
        Self {
            kind: ReadErrorKind::Nom(kind),
            position: input.location_offset(),
        }
    }

    fn append(_: Input, _: ErrorKind, other: Self) -> Self {
        other
    }
}

impl FromExternalError<Input<'_>, Self> for ReadError {
    fn from_external_error(_: Input, _: ErrorKind, error: Self) -> Self {
        error
    }
}

#[derive(Error, Debug)]
pub enum WriteError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("data too long")]
    DataTooLong,
}

#[derive(Error, Debug)]
pub enum FromSamplesError {
    #[error("channels are not all the same length")]
    InequalChannelLength,
    #[error("too many channels")]
    TooManyChannels,
}

pub trait WaveFileSample: FromSample<f64> + Sized + ToBytes
where
    <Self as ToBytes>::Bytes: IntoIterator<Item = u8>,
{
    const SAMPLE_FORMAT: Format;
    #[must_use]
    fn from_f64(value: f64) -> Self {
        Self::from_sample_(value)
    }
}

impl WaveFileSample for u8 {
    const SAMPLE_FORMAT: Format = Format::PulseCodeModulation;
}

impl WaveFileSample for i16 {
    const SAMPLE_FORMAT: Format = Format::PulseCodeModulation;
}

impl WaveFileSample for i32 {
    const SAMPLE_FORMAT: Format = Format::PulseCodeModulation;
}

impl WaveFileSample for i64 {
    const SAMPLE_FORMAT: Format = Format::PulseCodeModulation;
}

impl WaveFileSample for f32 {
    const SAMPLE_FORMAT: Format = Format::FloatingPoint;
}

impl WaveFileSample for f64 {
    const SAMPLE_FORMAT: Format = Format::FloatingPoint;
}

impl WaveFile {
    /// Create a new [`WaveFile`] from an iterable of samples and a sample rate, or [`None`] if the number of channels does not fit in a [`NonZeroU16`], because it was zero or more than [`u16::MAX`].
    /// # Panics
    /// Panics if the size of the sample type is too large to fit in a [`u16`], because there were more than [`u16::MAX`] channels.
    /// # Errors
    /// If the number of channels does not fit in a [`NonZeroU16`], because it is zero, or more than [`u16::MAX`], return [`FromSamplesError::TooManyChannels`].
    /// If the channels are not all the same length, return [`FromSamplesError::InequalChannelLength`].
    pub fn from_samples<T: WaveFileSample, C: IntoIterator<Item = f64>>(channels: impl IntoIterator<Item = C>, sample_rate: u32) -> Result<Self, FromSamplesError>
    where
        <T as ToBytes>::Bytes: IntoIterator<Item = u8>,
    {
        let format = T::SAMPLE_FORMAT;
        let mut channels = channels.into_iter().map(|channel| channel.into_iter().map(|sample| T::from_f64(sample).to_le_bytes())).collect_vec();
        let number_of_channels = u16::try_from(channels.len()).ok().and_then(NonZeroU16::new).ok_or(FromSamplesError::InequalChannelLength)?;
        let bytes_per_sample = u16::try_from(size_of::<T>()).expect("size of sample type is too large");
        let mut data = Vec::new();
        loop {
            let samples = channels.iter_mut().map(Iterator::next).collect_vec();
            if samples.iter().all(Option::is_none) {
                break;
            }
            let Some(samples) = samples.into_iter().collect::<Option<Vec<_>>>() else {
                return Err(FromSamplesError::InequalChannelLength);
            };
            data.extend(samples.into_iter().flatten());
        }

        Ok(Self {
            format,
            channels: number_of_channels,
            sample_rate,
            bytes_per_sample,
            data,
        })
    }

    #[must_use]
    pub const fn from_raw_data(data: Vec<u8>, format: Format, channels: NonZeroU16, sample_rate: u32, bytes_per_sample: u16) -> Self {
        Self {
            format,
            channels,
            sample_rate,
            bytes_per_sample,
            data,
        }
    }

    /// Write the [`WaveFile`] to a writer.
    /// # Errors
    /// Returns an [`WaveFileWriteError::Io`] if writing to the writer fails (from calls to [`Write::write_all`]), or [`WaveFileWriteError::DataTooLong`] if the data was longer than [`u32::MAX`]
    /// bytes.
    pub fn write(&self, writer: &mut impl Write) -> Result<(), WriteError> {
        const RIFF_DATA_LEN_PCM: usize = 4 + 4 + 4 + 2 + 2 + 4 + 4 + 2 + 2 + 4 + 4;
        const RIFF_DATA_LEN_FLOAT: usize = RIFF_DATA_LEN_PCM + 2 + 4 + 4 + 4;
        writer.write_all(b"RIFF")?;
        writer.write_all(
            &u32::try_from(if self.format == Format::FloatingPoint { RIFF_DATA_LEN_FLOAT } else { RIFF_DATA_LEN_PCM } + self.data.len())
                .map_err(|_| WriteError::DataTooLong)?
                .to_le_bytes(),
        )?;
        writer.write_all(b"WAVEfmt ")?;
        writer.write_all(&if self.format == Format::FloatingPoint { 18_u32 } else { 16_u32 }.to_le_bytes())?;
        writer.write_all(&(self.format as u16).to_le_bytes())?;
        writer.write_all(&self.channels.get().to_le_bytes())?;
        writer.write_all(&self.sample_rate.to_le_bytes())?;
        writer.write_all(&(self.sample_rate * u32::from(self.channels.get()) * u32::from(self.bytes_per_sample)).to_le_bytes())?;
        writer.write_all(&(self.bytes_per_sample).to_le_bytes())?;
        writer.write_all(&(self.bytes_per_sample * 8).to_le_bytes())?;
        if self.format == Format::FloatingPoint {
            writer.write_all(&0_u16.to_le_bytes())?;

            writer.write_all(b"fact")?;
            writer.write_all(&4_u32.to_le_bytes())?;
            writer.write_all(&u32::try_from(self.data.len() / self.bytes_per_sample as usize).map_err(|_| WriteError::DataTooLong)?.to_le_bytes())?;
        }
        writer.write_all(b"data")?;
        writer.write_all(&u32::try_from(self.data.len()).map_err(|_| WriteError::DataTooLong)?.to_le_bytes())?;
        writer.write_all(&self.data)?;
        Ok(())
    }

    /// Read a [`WaveFile`] from some bytes.
    /// # Errors
    /// Returns a [`ReadError`] if the file is not a valid wave file.
    pub fn read(bytes: &[u8]) -> Result<Self, ReadError> {
        complete(wave_file)(LocatedSpan::new(bytes)).map(|(_, wave_file)| wave_file).map_err(|error| match error {
            Err::Incomplete(_) => {
                // SAFETY: we called `complete`, so `Err::Incomplete` is impossible.
                unsafe { unreachable_unchecked() }
            }
            Err::Error(error) | Err::Failure(error) => error,
        })
    }
}

mod read {
    use std::num::NonZeroU16;

    use nom::{
        IResult,
        branch::{alt, permutation},
        bytes::complete::{tag, take},
        combinator::{all_consuming, consumed, map, map_res, opt, verify},
        multi::{length_data, length_value},
        number::complete::{le_u16, le_u32},
        sequence::{preceded, terminated, tuple},
    };
    use nom_locate::LocatedSpan;

    use super::{Format, ReadError, ReadErrorKind, WaveFile};

    pub type Input<'a> = LocatedSpan<&'a [u8]>;
    type FormatChunk = (Format, NonZeroU16, u32, u16, u16);

    fn format_chunk(input: Input) -> IResult<Input, FormatChunk, ReadError> {
        map_res::<Input, _, _, _, _, _, _>(
            preceded(
                tag(b"fmt "),
                length_value(
                    le_u32,
                    all_consuming(tuple((
                        consumed(le_u16),
                        consumed(le_u16),
                        le_u32,
                        consumed(le_u32),
                        le_u16,
                        le_u16,
                        map(opt(tag(&0_u16.to_le_bytes())), |extension| extension.is_some()),
                    ))),
                ),
            ),
            |((format_span, format), (channels_span, channels), sample_rate, (bytes_per_second_span, bytes_per_second), block_size, bits_per_sample, has_extension)| {
                if sample_rate * u32::from(block_size) != bytes_per_second {
                    return Err(ReadError {
                        kind: ReadErrorKind::DataRateMismatch,
                        position: bytes_per_second_span.location_offset(),
                    });
                }
                if channels != block_size / (bits_per_sample / 8) {
                    return Err(ReadError {
                        kind: ReadErrorKind::ChannelCountMismatch,
                        position: channels_span.location_offset(),
                    });
                }
                let Some(channels) = NonZeroU16::new(channels) else {
                    return Err(ReadError {
                        kind: ReadErrorKind::NoChannels,
                        position: channels_span.location_offset(),
                    });
                };
                let format = match format {
                    1 => Format::PulseCodeModulation,
                    3 => Format::FloatingPoint,
                    _ => {
                        return Err(ReadError {
                            kind: ReadErrorKind::FormatNotSupported,
                            position: format_span.location_offset(),
                        });
                    }
                };
                if format == Format::FloatingPoint && !has_extension {
                    return Err(ReadError {
                        kind: ReadErrorKind::MissingFormatChunkExtensionSize,
                        position: format_span.location_offset(),
                    });
                }

                Ok((format, channels, sample_rate, block_size, bits_per_sample))
            },
        )(input)
    }

    fn data_chunk(input: Input) -> IResult<Input, Input, ReadError> {
        preceded(
            tag(b"data"),
            alt((verify(length_data(le_u32), |data: &Input| data.len().is_multiple_of(2)), terminated(length_data(le_u32), take(1_usize)))),
        )(input)
    }

    fn fact_chunk(input: Input) -> IResult<Input, (Input, u32), ReadError> {
        preceded(tag(b"fact\x04\0\0\0"), consumed(le_u32))(input)
    }

    pub fn wave_file(input: Input) -> IResult<Input, WaveFile, ReadError> {
        all_consuming(map_res(
            preceded(tag(b"RIFF"), length_value(le_u32, preceded(tag(b"WAVE"), permutation((format_chunk, data_chunk, opt(fact_chunk)))))),
            |((format, channels, sample_rate, block_size, bits_per_sample), data, samples_length)| {
                if let Some((span, samples_length)) = samples_length {
                    if data.len() / (usize::from(bits_per_sample) / 8) != samples_length as usize {
                        return Err(ReadError {
                            kind: ReadErrorKind::FactChunkLengthMismatch,
                            position: span.location_offset(),
                        });
                    }
                    if data.len() != block_size as usize * samples_length as usize / usize::from(channels.get()) {
                        return Err(ReadError {
                            kind: ReadErrorKind::InvalidDataSize,
                            position: data.location_offset(),
                        });
                    }
                } else {
                    if format == Format::FloatingPoint {
                        return Err(ReadError {
                            kind: ReadErrorKind::MissingFactChunk,
                            position: input.len(),
                        });
                    }
                    if !data.len().is_multiple_of(block_size as usize) {
                        return Err(ReadError {
                            kind: ReadErrorKind::DataSizeNotMultipleOfBlockSize,
                            position: data.location_offset(),
                        });
                    }
                }

                Ok(WaveFile {
                    format,
                    channels,
                    sample_rate,
                    bytes_per_sample: bits_per_sample / 8,
                    data: data.into_fragment().to_vec(),
                })
            },
        ))(input)
    }
}
