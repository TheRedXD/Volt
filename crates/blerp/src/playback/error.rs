use cpal::{DevicesError, StreamError};
use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PlaybackError {
    #[error("Audio device error: {0}")]
    Device(#[from] DevicesError),

    #[error("Audio stream error: {0}")]
    Stream(#[from] StreamError),

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

pub type PlaybackResult<T> = Result<T, PlaybackError>;
