/// Collection of expected errors that can be encountered
/// by Volt.
#[derive(Debug, thiserror::Error)]
pub enum VoltError {
    /// Error encountered when a provided sample rate
    /// does fall within expected / supported bounds.
    #[error("Invalid bits per sample!")]
    InvalidSampleFormat,
    /// Error encountered when rendering frames.
    #[error("EFrame: {}", _0)]
    EFrame(#[from] eframe::Error),
    /// Error encountered when processing I/O.
    #[error("IO: {}", _0)]
    IO(#[from] std::io::Error),
    /// Error encountered when attempting to identify
    /// system hosts.
    #[error("System Host: {}", _0)]
    Host(#[from] cpal::HostUnavailable),
    /// Error encountered when attempting to identify
    /// system devices.
    #[error("System Device: {}", _0)]
    Devices(#[from] cpal::DevicesError),
    /// Error encountered when attempting to identify
    /// stream configs.
    #[error("Config not found")]
    Config,
    /// Error encountered when attempting to identify
    /// default stream configs.
    #[error("Stream Config: {}", _0)]
    DefaultStreamConfig(#[from] cpal::DefaultStreamConfigError),
    /// Error encountered when attempting to identify
    /// supported stream configs.
    #[error("Stream Config: {}", _0)]
    SupportedStreamConfig(#[from] cpal::SupportedStreamConfigsError),
    /// Error encountered when attempting to build
    /// a device stream.
    #[error("Build Stream: {}", _0)]
    BuildStream(#[from] cpal::BuildStreamError),
    /// Error encountered when attempting to play
    /// a device stream.
    #[error("Play Stream: {}", _0)]
    Play(#[from] cpal::PlayStreamError),
    /// Error encountered when attempting to create
    /// a audio decoder.
    #[error("Decoder: {}", _0)]
    Decoder(#[from] rodio::decoder::DecoderError),
}
