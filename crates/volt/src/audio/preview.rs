use blerp::{
    read::Reader,
    streaming::{AudioStream, DeviceManager, SampleBuffer, StreamCommand, StreamingError},
};
use cpal::StreamConfig;
use crossbeam_channel::{Receiver, Sender, TryRecvError, unbounded};
use itertools::Itertools;
use std::{
    fs::File,
    io,
    num::NonZeroUsize,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use tap::Tap;
use tracing::error;

#[derive(Debug, thiserror::Error)]
pub enum PreviewError {
    #[error("streaming error: {0}")]
    Streaming(#[from] StreamingError),
    #[error("no output audio device available")]
    NoOutputDevice,
    #[error("file system io error: {0}")]
    FileSystemIo(#[from] io::Error),
    #[error("error from Symphonia: {0}")]
    Symphonia(#[from] symphonia::core::errors::Error),
    #[error("audio system not initialized")]
    NotInitialized,
}

pub type PreviewResult<T> = Result<T, PreviewError>;

#[derive(Debug, Clone)]
pub enum PreviewCommand {
    PlayFile(PathBuf),
    Stop,
    Shutdown,
}

#[derive(Debug, Clone)]
pub struct PreviewData {
    pub duration: Duration,
    pub started_playing: Instant,
    pub path: Option<Arc<PathBuf>>,
}

impl PreviewData {
    pub fn progress(&self) -> Duration {
        self.started_playing.elapsed()
    }

    pub fn remaining(&self) -> Duration {
        self.duration - self.progress()
    }

    /// Return a value between 0 and 1 representing how much of the audio has played.
    pub fn percentage(&self) -> f32 {
        self.progress().as_secs_f32() / self.duration.as_secs_f32()
    }
}

pub struct Preview {
    command_tx: Sender<PreviewCommand>,
    data_rx: Receiver<Option<PreviewData>>,
    _worker_thread: JoinHandle<()>,
    data: Option<PreviewData>,
    is_running: Arc<AtomicBool>,
}

impl Preview {
    pub fn new() -> Self {
        let (command_tx, command_rx) = unbounded();
        let (data_tx, data_rx) = unbounded();
        let is_running = Arc::new(AtomicBool::new(true));

        let worker_thread = {
            let is_running = Arc::clone(&is_running);
            thread::spawn(move || {
                if let Err(error) = preview_worker(&command_rx, &data_tx, &is_running) {
                    error!("Preview worker error: {}", error);
                }
            })
        };

        Self {
            command_tx,
            data_rx,
            _worker_thread: worker_thread,
            data: None,
            is_running,
        }
    }

    pub fn play_file(&self, path: PathBuf) -> Result<(), PreviewError> {
        self.command_tx.send(PreviewCommand::PlayFile(path)).map_err(|_| PreviewError::NotInitialized)
    }

    pub fn stop(&mut self) -> Result<(), PreviewError> {
        self.data = None;
        self.command_tx.send(PreviewCommand::Stop).map_err(|_| PreviewError::NotInitialized)
    }

    pub fn data(&mut self) -> Option<&PreviewData> {
        if let Ok(data) = self.data_rx.try_recv() {
            self.data = data;
        }
        self.data.as_ref()
    }

    pub fn clear_data(&mut self) {
        self.data = None;
    }

    pub fn is_playing(&self) -> bool {
        self.data.is_some()
    }
}

impl Drop for Preview {
    fn drop(&mut self) {
        let _ = self.command_tx.send(PreviewCommand::Shutdown);
        self.is_running.store(false, Ordering::SeqCst);
    }
}

// TODO handle channel send/recv errors better (`let _ = ...`)
fn preview_worker(command_rx: &Receiver<PreviewCommand>, data_tx: &Sender<Option<PreviewData>>, is_running: &AtomicBool) -> PreviewResult<()> {
    let device_manager = DeviceManager::new()?;
    let Some(device) = device_manager.get_default_device() else {
        return Err(PreviewError::NoOutputDevice);
    };

    let buffer = Arc::new(SampleBuffer::new(44100 * 60, const { NonZeroUsize::new(2).unwrap() }));
    let (mut audio_stream, command_tx) = AudioStream::new(device.clone(), Arc::clone(&buffer), 44100, 512)?;
    let mut current_reader: Option<Reader> = None;

    let mut current_data = None;
    while is_running.load(Ordering::SeqCst) {
        if let Some(ref mut reader) = current_reader {
            buffer.write_frames(&reader.packet()?.collect_vec());
        }
        match command_rx.try_recv() {
            Ok(PreviewCommand::PlayFile(path)) => match Reader::new(File::open(&path)?) {
                Ok(reader) => {
                    current_data = Some(PreviewData {
                        duration: reader.duration(),
                        started_playing: Instant::now(),
                        path: Some(Arc::new(path)),
                    });
                    let _ = command_tx.send(StreamCommand::Start);
                    let _ = command_tx.send(StreamCommand::UpdateConfig(
                        audio_stream.config().tap_mut(|config| config.sample_rate.0 = reader.sample_rate().unwrap()),
                    ));
                    let _ = data_tx.send(current_data.clone());
                    current_reader = Some(reader);
                }
                Err(e) => {
                    error!("Failed to load audio file: {}", e);
                    current_data = None;
                    let _ = data_tx.send(None);
                }
            },
            Ok(PreviewCommand::Stop) => {
                let _ = command_tx.send(StreamCommand::Stop);
            }
            Err(TryRecvError::Empty) => {}                                           // No commands
            Ok(PreviewCommand::Shutdown) | Err(TryRecvError::Disconnected) => break, // Channel closed
        }

        if let Some(ref data) = current_data
            && data.started_playing.elapsed() >= data.duration
        {
            let _ = command_tx.send(StreamCommand::Stop);
            let _ = data_tx.send(None);
        }

        if let Err(error) = audio_stream.process_commands() {
            error!("error while processing commands: {}", error);
            break;
        }

        // Prevent busy waiting
        thread::sleep(Duration::from_millis(10));
    }

    Ok(())
}
