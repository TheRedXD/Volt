use std::{
    cmp,
    f64::consts::TAU,
    fmt::Debug,
    ops::Range,
    sync::{
        Arc, Mutex,
        atomic::{self, AtomicU64},
    },
    thread::{JoinHandle, park, sleep, spawn},
    time::Duration,
};

use cpal::traits::{DeviceTrait, StreamTrait};
use crossbeam_channel::Sender;
use itertools::Itertools;
use ringbuf::{
    HeapProd, HeapRb,
    traits::{Consumer, Observer, Producer, Split},
};

use crate::{
    SAMPLE_RATE,
    processing::time::{Beats, Samples, Tempo, Time, TimeSignature},
    streaming::{
        clip::{AudioClipData, Clip, ClipData, ClipTiming, ClipTimingBeats, ClipTimingSamples},
        track::Track,
    },
};

#[derive(Clone)]
pub struct Playlist {
    tracks: Arc<Vec<Track>>,
    pub time_signature: TimeSignature,
    pub tempo: Tempo,
    pub preview: Option<ClipTiming>,
}

pub struct PlaylistOutput {
    audio_engine: JoinHandle<()>,
    audio_engine_tx: Sender<AudioEngineMessage>,
    stream: cpal::Stream,
    playing: bool,
}

impl PlaylistOutput {
    fn play(&mut self) {
        self.stream.play().unwrap();
        self.audio_engine_tx.send(AudioEngineMessage::Play).unwrap();
        self.playing = true;
    }

    fn stop(&mut self) {
        self.stream.pause().unwrap();
        self.audio_engine_tx.send(AudioEngineMessage::Stop).unwrap();
        self.playing = false;
    }
}

enum AudioEngineMessage {
    Play,
    Stop,
    Seek(Samples),
    Update(Playlist),
}

pub struct PlaylistAudio {
    out: Option<PlaylistOutput>,
    playlist: Playlist,
    playhead: Arc<AtomicU64>,
}

impl PlaylistAudio {
    pub fn new() -> Self {
        Self {
            out: None,
            playlist: Playlist::new(),
            playhead: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn device_out(&mut self, device: &cpal::Device, config: &cpal::StreamConfig) -> &mut PlaylistOutput {
        let (mut master_tx, mut master_rx) = HeapRb::new(1024).split();

        let stream = device
            .build_output_stream(
                config,
                {
                    let playhead = Arc::clone(&self.playhead);
                    let channels = config.channels;
                    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                        let popped = master_rx.pop_slice(data) as u64;
                        playhead.fetch_add(popped / u64::from(channels), atomic::Ordering::Relaxed);
                    }
                },
                move |err| {
                    eprintln!("stream error: {err}");
                },
                None,
            )
            .unwrap();
        stream.pause().unwrap();
        let (audio_engine_tx, audio_engine_rx) = crossbeam_channel::unbounded();
        let audio_engine = {
            let playhead = Arc::clone(&self.playhead);
            let channels = config.channels;
            let initial = self.playlist.clone();
            spawn(move || {
                const AHEAD: Samples = Samples(1024.);
                let mut next: Samples = Samples::default();
                let mut playlist = initial;
                let mut playing = false;
                loop {
                    if let Ok(message) = audio_engine_rx.try_recv() {
                        match message {
                            AudioEngineMessage::Play => {
                                playing = true;
                            }
                            AudioEngineMessage::Stop => {
                                playing = false;
                            }
                            AudioEngineMessage::Seek(position) => {
                                playhead.store(position.u64(), atomic::Ordering::Relaxed);
                                next = position;
                            }
                            AudioEngineMessage::Update(new) => {
                                playlist = new;
                            }
                        }
                    }
                    if playing && let Some(preview) = playlist.preview {
                        playhead.update(atomic::Ordering::Relaxed, atomic::Ordering::Relaxed, |playhead| {
                            if playhead >= preview.as_samples(playlist.tempo).end.u64() {
                                next = preview.as_samples(playlist.tempo).start;
                                next.u64()
                            } else {
                                playhead
                            }
                        });
                    }
                    let playhead = playhead.load(atomic::Ordering::Relaxed);
                    match (next.0 - playhead as f64).partial_cmp(&AHEAD.0).unwrap() {
                        cmp::Ordering::Less => {
                            let vacant = master_tx.vacant_len() as f64;
                            let block = ClipTimingSamples {
                                start: next,
                                end: next + Samples(vacant),
                                offset: Samples(0.),
                            };
                            let mut buffer = vec![0.; vacant as usize];
                            for track in &*playlist.tracks {
                                for clip in &track.clips {
                                    let ClipTimingSamples { start, end, offset } = clip.timing.as_samples(playlist.tempo);
                                    let intersection = start.usize().max(block.start.usize())..end.usize().min(block.end.usize());
                                    if intersection.is_empty() {
                                        continue;
                                    }
                                    let destination = intersection.start - block.start.usize();
                                    let destination = destination..destination + intersection.len();
                                    let source = intersection.start + offset.usize() - start.usize();
                                    let source = source..source + intersection.len();
                                    match &clip.data {
                                        ClipData::Audio(AudioClipData { data }) => {
                                            if source.start >= data.len() {
                                                continue;
                                            }
                                            let source = source.start..source.end.clamp(0, data.len());
                                            for (buffer, data) in buffer.chunks_exact_mut(channels as usize).skip(destination.start).take(destination.len()).zip(&data[source]) {
                                                for sample in buffer {
                                                    *sample = (*data).mul_add(track.gain, *sample);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            master_tx.push_slice(&buffer);
                            next += Samples(vacant / f64::from(channels));
                        }
                        cmp::Ordering::Greater | cmp::Ordering::Equal => {
                            sleep(Duration::from_millis(5));
                        }
                    }
                }
            })
        };
        self.out.insert(PlaylistOutput {
            audio_engine,
            audio_engine_tx,
            stream,
            playing: false,
        })
    }

    pub fn play(&mut self) {
        if let Some(out) = &mut self.out {
            out.play();
        }
    }

    pub fn stop(&mut self) {
        if let Some(out) = &mut self.out {
            out.stop();
        }
    }

    pub fn playing(&self) -> bool {
        self.out.as_ref().is_some_and(|out| out.playing)
    }

    pub fn playhead(&self) -> Samples {
        Samples(self.playhead.load(atomic::Ordering::Relaxed) as f64)
    }

    pub fn seek(&self, position: Time) {
        if let Some(out) = &self.out {
            out.audio_engine_tx.send(AudioEngineMessage::Seek(position.samples(self.playlist.tempo))).unwrap();
        }
    }

    pub fn playlist(&self) -> &Playlist {
        &self.playlist
    }

    fn send_update(&self) {
        if let Some(out) = &self.out {
            out.audio_engine_tx.send(AudioEngineMessage::Update(self.playlist.clone())).unwrap();
        }
    }

     fn set_tempo(&mut self, tempo: Tempo) {
        self.playlist.tempo = tempo;
        self.send_update();
    }

    /// Update the tempo of the playlist and return the previous tempo.
    /// `update` receives the current tempo and should return the new tempo.
    pub fn update_tempo(&mut self, update: impl FnOnce(Tempo) -> Tempo) -> Tempo {
        let old = self.playlist.tempo;
        self.set_tempo(update(self.playlist.tempo));
        old
    }
}

impl Playlist {
    #[must_use]
    pub fn new() -> Self {
        Self {
            tracks: {
                let data = Arc::from(
                    (0..SAMPLE_RATE as u32)
                        .map(|index| {
                            let time = f64::from(index) / SAMPLE_RATE;
                            ((TAU * 440.0 * time).sin() * (-time * 4.).exp()) as f32
                        })
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                );

                Arc::new(vec![
                    Track {
                        clips: vec![
                            Clip {
                                data: ClipData::Audio(AudioClipData { data: Arc::clone(&data) }),
                                timing: ClipTiming::Beats(ClipTimingBeats {
                                    start: Beats(0.),
                                    end: Beats(1.),
                                    offset: Beats(0.),
                                }),
                            },
                            Clip {
                                data: ClipData::Audio(AudioClipData { data: Arc::clone(&data) }),
                                timing: ClipTiming::Beats(ClipTimingBeats {
                                    start: Beats(2.),
                                    end: Beats(3.),
                                    offset: Beats(0.),
                                }),
                            },
                        ],
                        gain: 1.,
                    },
                    Track {
                        clips: vec![Clip {
                            data: ClipData::Audio(AudioClipData { data: Arc::clone(&data) }),
                            timing: ClipTiming::Beats(ClipTimingBeats {
                                start: Beats(2.5),
                                end: Beats(3.5),
                                offset: Beats(0.),
                            }),
                        }],
                        gain: 1.,
                    },
                ])
            },
            time_signature: TimeSignature::default(),
            tempo: Tempo::default(),
            preview: Some(ClipTiming::Beats(ClipTimingBeats {
                start: Beats(0.),
                end: Beats(8.),
                offset: Beats(0.),
            })),
        }
    }

    #[must_use]
    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }
}

impl Default for Playlist {
    fn default() -> Self {
        Self::new()
    }
}
