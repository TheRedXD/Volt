use cpal::{BuildStreamError, DevicesError, PlayStreamError, StreamError, SupportedStreamConfigsError};
use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StreamingError {
    #[error("Audio device error: {0}")]
    Device(#[from] DevicesError),

    #[error("Audio stream error: {0}")]
    Stream(#[from] StreamError),

    #[error("Failed to build audio stream: {0}")]
    BuildStream(#[from] BuildStreamError),

    #[error("Failed to play audio stream: {0}")]
    PlayStream(#[from] PlayStreamError),

    #[error("Failed to get supported stream configs: {0}")]
    SupportedConfigs(#[from] SupportedStreamConfigsError),

    #[error("Unsupported audio format")]
    UnsupportedFormat,

    #[error("Buffer underrun")]
    BufferUnderrun,

    #[error("Sample rate conversion error: {0}")]
    SampleRateConversion(String),

    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Audio processing error: {0}")]
    Processing(String),
}

pub type StreamingResult<T> = Result<T, StreamingError>;
