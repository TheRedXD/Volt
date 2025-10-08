use cpal::{BuildStreamError, DevicesError, PlayStreamError, StreamError, SupportedStreamConfigsError};
use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StreamingError {
    #[error("audio device error: {0}")]
    Device(#[from] DevicesError),

    #[error("audio stream error: {0}")]
    Stream(#[from] StreamError),

    #[error("failed to build audio stream: {0}")]
    BuildStream(#[from] BuildStreamError),

    #[error("failed to play audio stream: {0}")]
    PlayStream(#[from] PlayStreamError),

    #[error("failed to get supported stream configs: {0}")]
    SupportedConfigs(#[from] SupportedStreamConfigsError),

    #[error("unsupported audio format")]
    UnsupportedFormat,

    #[error("buffer underrun")]
    BufferUnderrun,

    #[error("sample rate conversion error: {0}")]
    SampleRateConversion(String),

    #[error("io error: {0}")]
    Io(#[from] io::Error),

    #[error("audio processing error: {0}")]
    Processing(String),
}

pub type StreamingResult<T> = Result<T, StreamingError>;
