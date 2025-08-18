use blerp::{
    streaming::{buffer, AudioStream, DeviceManager, SampleBuffer, StreamCommand, StreamingError},
    utils::Channel,
    wavefile::WaveFile,
};
use crossbeam_channel::{unbounded, Receiver, Sender, TryRecvError};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use tracing::error;

#[derive(Debug, thiserror::Error)]
pub enum PreviewError {
    #[error("Streaming error: {0}")]
    Streaming(#[from] StreamingError),
    #[error("File error: {0}")]
    File(String),
    #[error("Audio system not initialized")]
    NotInitialized,
}

pub type PreviewResult<T> = Result<T, PreviewError>;

#[derive(Debug, Clone)]
pub enum PreviewCommand {
    PlayFile(PathBuf),
    Stop,
    Shutdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewState {
    Idle,
    Loading,
    Playing,
    Error,
}

#[derive(Debug, Clone)]
pub struct PreviewData {
    pub length: Option<Duration>,
    pub started_playing: Instant,
    pub current_file: Option<PathBuf>, // Store current file reference
}

pub struct Preview {
    command_tx: Sender<PreviewCommand>,
    data_rx: Receiver<PreviewData>,
    _worker_thread: JoinHandle<()>,
    pub current_preview_path: Option<PathBuf>,
    is_running: Arc<AtomicBool>,
}

impl Preview {
    pub fn new() -> PreviewResult<Self> {
        let (command_tx, command_rx) = unbounded();
        let (data_tx, data_rx) = unbounded();
        let is_running = Arc::new(AtomicBool::new(true));

        let worker_thread = {
            let is_running = is_running.clone();
            thread::spawn(move || {
                if let Err(e) = preview_worker(command_rx, data_tx, is_running) {
                    error!("Preview worker error: {}", e);
                }
            })
        };

        Ok(Self {
            command_tx,
            data_rx,
            _worker_thread: worker_thread,
            current_preview_path: None,
            is_running,
        })
    }

    pub fn play_file(&self, path: PathBuf) -> Result<(), PreviewError> {
        self.command_tx.send(PreviewCommand::PlayFile(path)).map_err(|_| PreviewError::NotInitialized)
    }

    pub fn stop(&self) -> Result<(), PreviewError> {
        self.command_tx.send(PreviewCommand::Stop).map_err(|_| PreviewError::NotInitialized)
    }

    pub fn get_data(&self) -> Option<PreviewData> {
        self.data_rx.try_recv().ok()
    }
}

impl Drop for Preview {
    fn drop(&mut self) {
        let _ = self.command_tx.send(PreviewCommand::Shutdown);
        self.is_running.store(false, Ordering::SeqCst);
    }
}

fn preview_worker(command_rx: Receiver<PreviewCommand>, data_tx: Sender<PreviewData>, is_running: Arc<AtomicBool>) -> PreviewResult<()> {
    let device_manager = DeviceManager::new()?;
    let device = device_manager.get_default_device().ok_or_else(|| PreviewError::File("No audio device available".to_string()))?.clone();

    let buffer = Arc::new(SampleBuffer::new(44100 * 60, Channel::Stereo));
    let (mut audio_stream, stream_tx) = AudioStream::new(device, buffer.clone(), 44100, 512)?;

    let mut current_data: Option<PreviewData> = None;
    let mut is_playing = false;
    while is_running.load(Ordering::SeqCst) {
        match command_rx.try_recv() {
            Ok(PreviewCommand::PlayFile(path)) => {
                let _ = stream_tx.send(StreamCommand::Stop);

                match load_audio_file(&path, buffer.clone()) {
                    Ok(length) => {
                        current_data = Some(PreviewData {
                            length: Some(length),
                            started_playing: Instant::now(),
                            current_file: Some(path),
                        });

                        let _ = stream_tx.send(StreamCommand::Start);
                    }
                    Err(e) => {
                        eprintln!("Failed to load audio file: {}", e);
                        current_data = None;
                    }
                }
            }
            Ok(PreviewCommand::Stop) => {
                let _ = stream_tx.send(StreamCommand::Stop);
                current_data = None;
            }
            Ok(PreviewCommand::Shutdown) => break,
            Err(TryRecvError::Empty) => {}            // No commands
            Err(TryRecvError::Disconnected) => break, // Channel closed
        }

        if is_playing && current_data.is_some() {
            let data = current_data.as_ref().unwrap();
            if let Some(length) = data.length {
                let elapsed = data.started_playing.elapsed();
                if elapsed >= length {
                    // Audio has finished playing
                    let _ = stream_tx.send(StreamCommand::Stop);
                    current_data = None;
                    is_playing = false;
                }
            }
        }

        if is_playing && let Some(ref data) = current_data {
            let _ = data_tx.send(data.clone());
        }

        if let Err(_) = audio_stream.process_commands() {
            break;
        }

        // Prevent busy waiting
        thread::sleep(Duration::from_millis(10));
    }

    Ok(())
}

fn load_audio_file(path: &PathBuf, buffer: Arc<SampleBuffer>) -> Result<Duration, String> {
    let file_data = std::fs::read(path).map_err(|e| format!("Failed to read file: {}", e))?;

    let wave_file = WaveFile::read(&file_data).map_err(|e| format!("Failed to parse WAV file: {:?}", e))?;

    let samples = wave_data_to_samples(&wave_file)?;

    let sample_count = samples.len() / wave_file.channels.get() as usize;
    let duration = Duration::from_secs_f64(sample_count as f64 / wave_file.sample_rate as f64);

    buffer.clear();
    buffer.write_frames(&samples);

    Ok(duration)
}

fn wave_data_to_samples(wave_file: &WaveFile) -> Result<Vec<f32>, String> {
    use blerp::wavefile::Format as Fmt;

    let mut samples = Vec::new();
    let data = &wave_file.data;
    let bytes_per_sample = wave_file.bytes_per_sample as usize;

    match wave_file.format {
        Fmt::PulseCodeModulation => {
            match bytes_per_sample {
                1 => {
                    // u8 PCM
                    for chunk in data.chunks(1) {
                        let sample = chunk[0] as f32 / 128.0 - 1.0;
                        samples.push(sample);
                    }
                }
                2 => {
                    // i16 PCM
                    for chunk in data.chunks(2) {
                        let sample = i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / 32768.0;
                        samples.push(sample);
                    }
                }
                4 => {
                    // i32 PCM
                    for chunk in data.chunks(4) {
                        let sample = i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) as f32 / 2147483648.0;
                        samples.push(sample);
                    }
                }
                _ => return Err(format!("Unsupported PCM bit depth: {}", bytes_per_sample * 8)),
            }
        }
        Fmt::FloatingPoint => {
            match bytes_per_sample {
                4 => {
                    // f32
                    for chunk in data.chunks(4) {
                        let sample = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                        samples.push(sample);
                    }
                }
                8 => {
                    // f64
                    for chunk in data.chunks(8) {
                        let sample = f64::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6], chunk[7]]) as f32;
                        samples.push(sample);
                    }
                }
                _ => return Err(format!("Unsupported float bit depth: {}", bytes_per_sample * 8)),
            }
        }
    }

    Ok(samples)
}
