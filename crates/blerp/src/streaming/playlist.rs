use std::{
    cmp,
    f64::consts::TAU,
    fs::File,
    io,
    path::Path,
    range::Range,
    sync::{
        Arc, Mutex, atomic::{self, AtomicU64}
    },
    thread::{JoinHandle, sleep, spawn},
    time::Duration,
};

use cpal::traits::{DeviceTrait, StreamTrait};
use crossbeam_channel::Sender;
use itertools::Itertools;
use ringbuf::{
    HeapProd, HeapRb,
    traits::{Consumer, Observer, Producer, Split},
};
use symphonia::core::{
    audio::Signal,
    errors::Error as SymphoniaError,
    formats::{SeekMode, SeekTo, SeekedTo},
    units::Time as SymphoniaTime,
};
use tap::Conv;

use crate::{
    SAMPLE_RATE,
    processing::time::{Beats, Samples, Tempo, Time, TimeSignature},
    read::Reader,
    streaming::{
        clip::{AudioClipData, Clip, ClipData, ClipTiming, ClipTimingBeats, ClipTimingSamples, SymphoniaClipData},
        track::Track,
    },
};

#[derive(Clone)]
pub struct Playlist {
    tracks: Vec<Track>,
    pub time_signature: TimeSignature,
    pub tempo: Tempo,
    pub preview: Option<ClipTiming>,
}

pub struct PlaylistOutput {
    audio_engine: JoinHandle<()>,
    audio_engine_tx: Sender<AudioEngineMessage>,
    stream: cpal::Stream, 
    playing: bool,
    stream_playing: Arc<atomic::AtomicBool>,
}

impl PlaylistOutput {
    fn play(&mut self) {
        self.stream_playing.store(true, atomic::Ordering::Relaxed);
        self.audio_engine_tx.send(AudioEngineMessage::Play).unwrap();
        self.playing = true;
    }

    fn stop(&mut self) {
        self.stream_playing.store(false, atomic::Ordering::Relaxed);
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
        let (mut master_tx, mut master_rx) = HeapRb::new(2048*1024).split();
        let master_rx = Arc::new(Mutex::new(master_rx));
        let stream_playing = Arc::new(atomic::AtomicBool::new(false));
        
        let stream = device
            .build_output_stream(
                config,
                {
                    let playhead = Arc::clone(&self.playhead);
                    let channels = config.channels;
                    let master_rx = Arc::clone(&master_rx);
                    let stream_playing = Arc::clone(&stream_playing);
                    
                    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                        if stream_playing.load(atomic::Ordering::Relaxed) {
                            if let Ok(mut rx) = master_rx.try_lock() {
                                let popped = rx.pop_slice(data) as u64;
                                playhead.fetch_add(popped / u64::from(channels), atomic::Ordering::Relaxed);
                                data[popped as usize..].fill(0.0);
                            } else {
                                data.fill(0.0);
                            }
                        } else {
                            data.fill(0.0);
                        }
                    }
                },
                move |err| {
                    eprintln!("stream error: {err}");
                },
                None,
            )
            .unwrap();
            
        stream.play().unwrap();

        let (audio_engine_tx, audio_engine_rx) = crossbeam_channel::unbounded();
        let audio_engine = {
            let playhead = Arc::clone(&self.playhead);
            let channels = config.channels;
            let initial = self.playlist.clone();
            let master_rx = Arc::clone(&master_rx);
            
            spawn(move || {
                let mut next: Samples = Samples::default();
                let mut playlist = initial;
                let mut playing = false;
                
                loop {
                    while let Ok(message) = audio_engine_rx.try_recv() {
                        match message {
                            AudioEngineMessage::Play => playing = true,
                            AudioEngineMessage::Stop => playing = false,
                            AudioEngineMessage::Seek(position) => {
                                playhead.store(position.u64(), atomic::Ordering::Relaxed);
                                next = position;
                                if let Ok(mut rx) = master_rx.lock() {
                                    rx.clear();
                                }
                            }
                            AudioEngineMessage::Update(new) => playlist = new,
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

                    let current_playhead = playhead.load(atomic::Ordering::Relaxed);
                    
                    if playing {
                        let buffered_frames = next.0 - current_playhead as f64;
                        let target_buffer_frames = SAMPLE_RATE as f64 * 1.0; 

                        if buffered_frames < target_buffer_frames {
                            let vacant_frames = master_tx.vacant_len() / channels as usize;
                            
                            let chunk_frames = vacant_frames.min(SAMPLE_RATE as usize / 2);
                            
                            if chunk_frames > 0 {
                                let block = ClipTimingSamples {
                                    start: next,
                                    end: next + Samples(chunk_frames as f64),
                                    offset: Samples(0.),
                                };
                                let mut buffer = vec![0.; chunk_frames * channels as usize];
                                
                                for track in &mut playlist.tracks {
                                    for clip in &mut track.clips {
                                        let ClipTimingSamples { start, end, offset } = clip.timing.as_samples(playlist.tempo);
                                        let intersection = start.usize().max(block.start.usize())..end.usize().min(block.end.usize());
                                        if intersection.is_empty() {
                                            continue;
                                        }
                                        let destination = intersection.start - block.start.usize();
                                        let destination = destination..destination + intersection.len();
                                        let source = intersection.start + offset.usize() - start.usize();
                                        let source = Range::from(source..source + intersection.len());
                                        
                                        match &mut clip.data {
                                            ClipData::Audio(AudioClipData { data }) => {
                                                if source.start >= data.len() { continue; }
                                                let source_range = source.start..source.end.clamp(0, data.len());
                                                for (buffer, data) in buffer.chunks_exact_mut(channels as usize).skip(destination.start).take(destination.len()).zip(&data[source_range]) {
                                                    for sample in buffer {
                                                        *sample = (*data).mul_add(track.gain, *sample);
                                                    }
                                                }
                                            }
                                            ClipData::Symphonia(data) => {
                                                if source.start as u64 >= data.decoder.codec_params().n_frames.unwrap_or(u64::MAX) {
                                                    continue;
                                                }
    
                                                let SeekedTo { required_ts, actual_ts, .. } = match data.reader.format_reader.seek(
                                                    SeekMode::Accurate,
                                                    SeekTo::Time {
                                                        time: SymphoniaTime::from(source.start as f64 / SAMPLE_RATE),
                                                        track_id: data.reader.format_reader.tracks()[data.track].id.into(),
                                                    },
                                                ) {
                                                    Ok(res) => res,
                                                    Err(e) => {
                                                        eprintln!("Seek error: {e}");
                                                        continue;
                                                    }
                                                };

                                                data.decoder.reset();

                                                let time_base = data.decoder.codec_params().time_base.unwrap();
                                                let error_secs = time_base.calc_time(required_ts.saturating_sub(actual_ts)).conv::<Duration>().as_secs_f64();
                                                
                                                let mut skip_frames = (error_secs * SAMPLE_RATE).round() as usize;
                                                let mut needed = source.end - source.start;
                                                let mut decoded = Vec::<f32>::with_capacity(needed);
                                                
                                                loop {
                                                    let packet = match data.reader.format_reader.next_packet() {
                                                        Ok(packet) => packet,
                                                        Err(SymphoniaError::IoError(error)) if error.kind() == io::ErrorKind::UnexpectedEof => break,
                                                        Err(_) => break,
                                                    };

                                                    let source_audio = match data.decoder.decode(&packet) {
                                                        Ok(audio) => audio,
                                                        Err(SymphoniaError::DecodeError(_)) => {
                                                            data.decoder.reset();
                                                            continue;
                                                        }
                                                        Err(_) => break,
                                                    };

                                                    let mut dest_buf = source_audio.make_equivalent::<f32>();
                                                    source_audio.convert(&mut dest_buf);
                                                    
                                                    let chan = dest_buf.chan(0);
                                                    let to_skip = skip_frames.min(chan.len());
                                                    skip_frames -= to_skip;
                                                    
                                                    let take_len = needed.min(chan.len() - to_skip);
                                                    decoded.extend(chan.iter().skip(to_skip).take(take_len));
                                                    needed -= take_len;
                                                    
                                                    if needed == 0 {
                                                        break;
                                                    }
                                                }

                                                for (buffer, data) in buffer.chunks_exact_mut(channels as usize).skip(destination.start).take(destination.len()).zip(&decoded) {
                                                    for sample in buffer {
                                                        *sample = (*data).mul_add(track.gain, *sample);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                master_tx.push_slice(&buffer);
                                next += Samples(chunk_frames as f64);
                            } else {
                                sleep(Duration::from_millis(1));
                            }
                        } else {
                            sleep(Duration::from_millis(1));
                        }
                    } else {
                        sleep(Duration::from_millis(1));
                    }
                }
            })
        };
        self.out.insert(PlaylistOutput {
            audio_engine,
            audio_engine_tx,
            stream,
            playing: false,
            stream_playing,
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

    pub fn update_playlist(&mut self, update: impl FnOnce(&mut Playlist)) {
        update(&mut self.playlist);
        if let Some(out) = &self.out {
            out.audio_engine_tx.send(AudioEngineMessage::Update(self.playlist.clone())).unwrap();
        }
    }

    /// Update the tempo of the playlist and return the previous tempo.
    /// `update` receives the current tempo and should return the new tempo.
    pub fn update_tempo(&mut self, update: impl FnOnce(Tempo) -> Tempo) -> Tempo {
        let old = self.playlist.tempo;
        self.update_playlist(|playlist| playlist.tempo = update(playlist.tempo));
        old
    }

    pub fn update_beats_per_measure(&mut self, update: impl FnOnce(u32) -> u32) -> u32 {
        let old = self.playlist.time_signature.beats_per_measure;
        self.update_playlist(|playlist| playlist.time_signature.beats_per_measure = update(playlist.time_signature.beats_per_measure));
        old
    }

    pub fn update_beat_value(&mut self, update: impl FnOnce(u32) -> u32) -> u32 {
        let old = self.playlist.time_signature.beat_value;
        self.update_playlist(|playlist| playlist.time_signature.beat_value = update(playlist.time_signature.beat_value));
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

                vec![
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
                ]
            },
            time_signature: TimeSignature::default(),
            tempo: Tempo::default(),
            preview: None,
        }
    }

    pub fn set_track_gain(&mut self, index: usize, gain: f32) {
        self.tracks[index].gain = gain;
    }

    pub fn add_clips(&mut self, track: usize, path: Arc<Path>, start: Time) {
        let clips = SymphoniaClipData::from_path(path);
        self.tracks.resize((track + clips.len()).max(self.tracks.len()), Track { clips: Vec::new(), gain: 1. });
        for (track, clip) in self.tracks.iter_mut().skip(track).zip(clips) {
            track.clips.push(Clip {
                timing: ClipTiming::Samples(ClipTimingSamples {
                    start: start.samples(self.tempo),
                    end: start.samples(self.tempo)
                        + Samples(
                            clip.decoder
                                .codec_params()
                                .time_base
                                .unwrap()
                                .calc_time(clip.decoder.codec_params().n_frames.unwrap())
                                .conv::<Duration>()
                                .as_secs_f64()
                                * SAMPLE_RATE,
                        ),
                    offset: Samples(0.),
                }),
                data: ClipData::Symphonia(clip),
            });
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
