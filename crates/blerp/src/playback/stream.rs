use super::{
    buffer::SampleBuffer,
    device::Device,
    error::{PlaybackError, PlaybackResult},
};
use cpal::{
    traits::{DeviceTrait, StreamTrait},
    BufferSize, SampleFormat, SampleRate, Stream, StreamConfig, StreamError,
};
use crossbeam_channel::{Receiver, Sender, TryRecvError};
use parking_lot::Mutex;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};
use tracing::{debug, error, info, warn};

/// Commands for controlling the audio stream
#[derive(Debug, Clone)]
pub enum StreamCommand {
    /// Start the audio stream
    Start,
    /// Stop the audio stream
    Stop,
    /// Set master volume (0.0 to 1.0)
    SetVolume(f32),
    /// Update stream configuration
    UpdateConfig(StreamConfig),
    /// Shutdown the stream completely
    Shutdown,
}

/// Current state of the audio stream
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamState {
    Stopped,
    Starting,
    Running,
    Stopping,
    Error,
}

/// Audio stream statistics for monitoring
#[derive(Debug, Default)]
pub struct StreamStats {
    pub frames_processed: AtomicU64,
    pub underruns: AtomicU64,
    pub overruns: AtomicU64,
    pub last_callback_duration_us: AtomicU64,
}

impl StreamStats {
    pub fn reset(&self) {
        self.frames_processed.store(0, Ordering::Relaxed);
        self.underruns.store(0, Ordering::Relaxed);
        self.overruns.store(0, Ordering::Relaxed);
        self.last_callback_duration_us.store(0, Ordering::Relaxed);
    }
}

pub struct AudioStream {
    device: Device,
    config: StreamConfig,
    stream: Option<Stream>,
    buffer: Arc<SampleBuffer>,

    // Control channels
    command_rx: Receiver<StreamCommand>,
    _command_tx: Sender<StreamCommand>, // Keep sender alive

    // Stream state
    state: Arc<Mutex<StreamState>>,
    is_running: Arc<AtomicBool>,
    volume: Arc<Mutex<f32>>,

    // Performance monitoring
    stats: Arc<StreamStats>,

    // Audio parameters
    sample_rate: u32,
    channel_count: usize,
}

impl AudioStream {
    /// Create a new audio stream
    ///
    /// # Errors
    /// Returns an error if the device doesn't support the requested configuration
    /// or if there's an issue setting up the audio stream.
    pub fn new(device: Device, buffer: Arc<SampleBuffer>, sample_rate: u32, buffer_size: usize) -> PlaybackResult<(Self, Sender<StreamCommand>)> {
        let (command_tx, command_rx) = crossbeam_channel::unbounded();

        // Get optimal configuration for the device
        let config = Self::get_optimal_config(&device, sample_rate, buffer.channels())?;

        info!(
            "Creating audio stream: device='{}', sample_rate={}, channels={}, buffer_size={}",
            device.name, config.sample_rate.0, config.channels, buffer_size
        );

        let stream = Self {
            device,
            config: config.clone(),
            stream: None,
            buffer,
            command_rx,
            _command_tx: command_tx.clone(),
            state: Arc::new(Mutex::new(StreamState::Stopped)),
            is_running: Arc::new(AtomicBool::new(false)),
            volume: Arc::new(Mutex::new(1.0)),
            stats: Arc::new(StreamStats::default()),
            sample_rate: config.sample_rate.0,
            channel_count: config.channels as usize,
        };

        Ok((stream, command_tx))
    }

    /// Start the audio stream
    ///
    /// # Errors
    /// Returns an error if the audio stream cannot be created or started.
    pub fn start(&mut self) -> PlaybackResult<()> {
        let mut state = self.state.lock();
        if *state == StreamState::Running {
            debug!("Stream already running");
            return Ok(());
        }

        *state = StreamState::Starting;
        drop(state);

        info!("Starting audio stream");

        // Create the CPAL stream
        let stream = self.create_cpal_stream()?;

        // Start the stream
        stream.play().map_err(PlaybackError::PlayStream)?;

        self.stream = Some(stream);
        self.is_running.store(true, Ordering::SeqCst);
        *self.state.lock() = StreamState::Running;
        self.stats.reset();

        info!("Audio stream started successfully");
        Ok(())
    }

    /// Stop the audio stream
    ///
    /// # Errors
    /// Returns an error if there's an issue stopping the audio stream,
    /// though this is unlikely in practice.
    pub fn stop(&mut self) -> PlaybackResult<()> {
        let mut state = self.state.lock();
        if *state == StreamState::Stopped {
            debug!("Stream already stopped");
            return Ok(());
        }

        *state = StreamState::Stopping;
        drop(state);

        info!("Stopping audio stream");

        self.is_running.store(false, Ordering::SeqCst);

        if let Some(stream) = self.stream.take() {
            // CPAL streams stop automatically when dropped
            drop(stream);
        }

        *self.state.lock() = StreamState::Stopped;

        info!("Audio stream stopped");
        Ok(())
    }

    /// Process stream commands (call this regularly from a control thread)
    ///
    /// # Errors
    /// Returns an error if there's an issue processing a command (e.g., starting or stopping the stream).
    /// Returns `Ok(false)` when the stream should be shut down.
    pub fn process_commands(&mut self) -> PlaybackResult<bool> {
        match self.command_rx.try_recv() {
            Ok(command) => {
                match command {
                    StreamCommand::Start => {
                        self.start()?;
                    }
                    StreamCommand::Stop => {
                        self.stop()?;
                    }
                    StreamCommand::SetVolume(volume) => {
                        self.set_volume(volume);
                    }
                    StreamCommand::UpdateConfig(config) => {
                        self.update_config(config)?;
                    }
                    StreamCommand::Shutdown => {
                        self.stop()?;
                        return Ok(false); // Signal shutdown
                    }
                }
                Ok(true)
            }
            Err(TryRecvError::Empty) => Ok(true),
            Err(TryRecvError::Disconnected) => {
                warn!("Command channel disconnected, shutting down stream");
                self.stop()?;
                Ok(false)
            }
        }
    }

    /// Set master volume (0.0 to 1.0)
    pub fn set_volume(&self, volume: f32) {
        let clamped_volume = volume.clamp(0.0, 1.0);
        *self.volume.lock() = clamped_volume;
        debug!("Volume set to {:.2}", clamped_volume);
    }

    /// Get current volume
    #[must_use]
    pub fn get_volume(&self) -> f32 {
        *self.volume.lock()
    }

    /// Update stream configuration (requires restart)
    ///
    /// # Errors
    /// Returns an error if the new configuration is not supported by the device
    /// or if there's an issue restarting the stream with the new configuration.
    pub fn update_config(&mut self, config: StreamConfig) -> PlaybackResult<()> {
        let was_running = self.is_running.load(Ordering::SeqCst);

        if was_running {
            self.stop()?;
        }

        self.config = config;
        self.sample_rate = self.config.sample_rate.0;
        self.channel_count = self.config.channels as usize;

        if was_running {
            self.start()?;
        }

        Ok(())
    }

    /// Get current stream state
    #[must_use]
    pub fn get_state(&self) -> StreamState {
        *self.state.lock()
    }

    /// Check if stream is running
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    /// Get stream statistics
    #[must_use]
    pub fn get_stats(&self) -> (u64, u64, u64, u64) {
        (
            self.stats.frames_processed.load(Ordering::Relaxed),
            self.stats.underruns.load(Ordering::Relaxed),
            self.stats.overruns.load(Ordering::Relaxed),
            self.stats.last_callback_duration_us.load(Ordering::Relaxed),
        )
    }

    /// Get optimal configuration for the device
    fn get_optimal_config(device: &Device, target_sample_rate: u32, target_channels: usize) -> PlaybackResult<StreamConfig> {
        let supported_configs = device.cpal_device.supported_output_configs().map_err(PlaybackError::SupportedConfigs)?;

        // Find the best matching configuration
        let mut best_config = None;
        let mut best_score = f32::MIN;

        for config_range in supported_configs {
            let channels = config_range.channels() as usize;
            let min_rate = config_range.min_sample_rate().0;
            let max_rate = config_range.max_sample_rate().0;

            // Skip if channel count doesn't match and can't be handled
            if channels != target_channels && channels < target_channels {
                continue;
            }

            // Check if target sample rate is supported
            let supported_rate = if target_sample_rate >= min_rate && target_sample_rate <= max_rate {
                target_sample_rate
            } else if target_sample_rate < min_rate {
                min_rate
            } else {
                max_rate
            };

            // Score configuration (prefer exact matches)
            let mut score = 0.0f32;

            // Sample rate score (higher is better)
            #[allow(clippy::cast_precision_loss, reason = "Rates will be inside of a f32 mantissa")]
            let rate_diff = (target_sample_rate as f32 - supported_rate as f32).abs();
            score += 1000.0 - rate_diff;

            // Channel count score (exact match is best)
            match channels.cmp(&target_channels) {
                std::cmp::Ordering::Equal => score += 500.0,
                std::cmp::Ordering::Greater => score += 100.0,
                std::cmp::Ordering::Less => (),
            }

            // Prefer f32 sample format
            if config_range.sample_format() == SampleFormat::F32 {
                score += 200.0;
            }

            if score > best_score {
                best_score = score;
                best_config = Some((config_range, supported_rate));
            }
        }

        let (config_range, sample_rate) = best_config.ok_or(PlaybackError::UnsupportedFormat)?;

        let config = StreamConfig {
            channels: config_range.channels(),
            sample_rate: SampleRate(sample_rate),
            buffer_size: BufferSize::Fixed(512), // Start with a reasonable default
        };

        debug!(
            "Selected audio config: sample_rate={}, channels={}, format={:?}",
            config.sample_rate.0,
            config.channels,
            config_range.sample_format()
        );

        Ok(config)
    }

    /// Create the actual CPAL stream
    fn create_cpal_stream(&self) -> PlaybackResult<Stream> {
        let buffer = self.buffer.clone();
        let volume = self.volume.clone();
        let stats = self.stats.clone();
        let is_running = self.is_running.clone();
        let channels = self.channel_count;

        // Audio callback closure
        let callback = move |data: &mut [f32], _info: &cpal::OutputCallbackInfo| {
            let start_time = std::time::Instant::now();

            if !is_running.load(Ordering::SeqCst) {
                // Fill with silence if not running
                data.fill(0.0);
                return;
            }

            let frames_requested = data.len() / channels;
            let frames_read = buffer.read_frames(data);

            // Apply volume
            let current_volume = *volume.lock();
            if (current_volume - 1.0).abs() > f32::EPSILON {
                for sample in data.iter_mut() {
                    *sample *= current_volume;
                }
            }

            // Handle underruns
            if frames_read < frames_requested {
                // Fill remaining with silence
                let samples_read = frames_read * channels;
                data[samples_read..].fill(0.0);

                stats.underruns.fetch_add(1, Ordering::Relaxed);

                if frames_read == 0 {
                    // Complete underrun
                    warn!("Audio buffer underrun: no data available");
                }
            }

            // Update statistics
            stats.frames_processed.fetch_add(frames_read as u64, Ordering::Relaxed);
            let duration_us = start_time.elapsed().as_micros();
            #[allow(clippy::cast_possible_truncation, reason = "Callback duration never will be over the size of an u64")]
            stats.last_callback_duration_us.store(duration_us as u64, Ordering::Relaxed);
        };

        // Error callback
        let error_callback = {
            let _state = self.state.clone();
            move |err: StreamError| {
                error!("Audio stream error: {}", err);
            }
        };

        // Build the stream
        let stream = self
            .device
            .cpal_device
            .build_output_stream(&self.config, callback, error_callback, None)
            .map_err(PlaybackError::BuildStream)?;

        Ok(stream)
    }
}

impl Drop for AudioStream {
    fn drop(&mut self) {
        if let Err(e) = self.stop() {
            error!("Error stopping audio stream during drop: {}", e);
        }
    }
}

pub type F32AudioStream = AudioStream;
