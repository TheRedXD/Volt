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

pub struct Playlist {
    playhead: Arc<AtomicU64>,
    tracks: Arc<Vec<Track>>,
    out: Option<PlaylistOutput>,
    pub time_signature: TimeSignature,
    pub tempo: Arc<Mutex<Tempo>>,
    pub preview: Arc<Mutex<Option<ClipTiming>>>,
}

impl Debug for Playlist {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Playlist")
            .field("playhead", &self.playhead.load(atomic::Ordering::Relaxed))
            .field("tracks", &[self.tracks.len()])
            .finish_non_exhaustive()
    }
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
        self.playing = true;
    }

    fn pause(&mut self) {
        self.stream.pause().unwrap();
        self.audio_engine_tx.send(AudioEngineMessage::Pause).unwrap();
        self.playing = false;
    }

    fn stop(&mut self) {
        self.pause();
        self.audio_engine_tx.send(AudioEngineMessage::Seek(Samples(0.))).unwrap();
    }
}

enum AudioEngineMessage {
    Play,
    Pause,
    Seek(Samples),
}

impl Playlist {
    #[must_use]
    pub fn new() -> Self {
        Self {
            playhead: Arc::new(AtomicU64::new(0)),
            tracks: {
                let wave = |time: f64| (TAU * 440.0 * time).sin();
                let data = Arc::from(
                    (0..SAMPLE_RATE as u32)
                        .flat_map(|index| [wave(index as f64 / SAMPLE_RATE as f64) as f32; 2])
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
                    },
                ])
            },
            out: None,
            time_signature: TimeSignature::default(),
            tempo: Arc::new(Mutex::new(Tempo::default())),
            preview: Arc::new(Mutex::new(Some(ClipTiming::Beats(ClipTimingBeats {
                start: Beats(0.),
                end: Beats(8.),
                offset: Beats(0.),
            })))),
        }
    }

    pub fn device_out(&mut self, device: &cpal::Device, config: &cpal::StreamConfig) -> &mut PlaylistOutput {
        let (mut master_tx, mut master_rx) = HeapRb::new(1024).split();

        let stream = device
            .build_output_stream(
                config,
                {
                    let playhead = Arc::clone(&self.playhead);
                    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                        let popped = master_rx.pop_slice(data) as u64;
                        playhead.fetch_add(popped, atomic::Ordering::Relaxed);
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
            let tracks = Arc::clone(&self.tracks);
            let tempo = Arc::clone(&self.tempo);
            let preview = Arc::clone(&self.preview);
            spawn(move || {
                const AHEAD: Samples = Samples(1024.);
                let mut next: Samples = Samples::default();
                let mut engine_tempo = Tempo::default();
                let mut engine_preview = *preview.lock().unwrap();
                loop {
                    if let Ok(message) = audio_engine_rx.try_recv() {
                        match message {
                            AudioEngineMessage::Play => {}
                            AudioEngineMessage::Pause => {}
                            AudioEngineMessage::Seek(position) => {
                                playhead.store(position.u64(), atomic::Ordering::Relaxed);
                                next = position;
                            }
                        }
                    }
                    if let Ok(tempo) = tempo.try_lock() {
                        engine_tempo = *tempo;
                    }
                    if let Ok(preview) = preview.try_lock() {
                        engine_preview = *preview
                    }
                    if let Some(preview) = engine_preview {
                        playhead.update(atomic::Ordering::Relaxed, atomic::Ordering::Relaxed, |playhead| {
                            if playhead >= preview.as_samples(engine_tempo).end.u64() {
                                next = preview.as_samples(engine_tempo).start;
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
                            for track in &*tracks {
                                for clip in &track.clips {
                                    let ClipTimingSamples { start, end, offset } = clip.timing.as_samples(engine_tempo);
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
                                            for (buffer, data) in buffer[destination].iter_mut().zip(&data[source]) {
                                                *buffer += *data;
                                            }
                                        }
                                    }
                                }
                            }
                            master_tx.push_slice(&buffer);
                            next += Samples(vacant);
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

    #[must_use]
    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }

    pub fn play(&mut self) {
        if let Some(out) = &mut self.out {
            out.play();
        }
    }

    pub fn pause(&mut self) {
        if let Some(out) = &mut self.out {
            out.pause();
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
}

impl Default for Playlist {
    fn default() -> Self {
        Self::new()
    }
}
