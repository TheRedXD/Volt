use std::{
    collections::HashMap,
    f64::consts::TAU,
    io,
    path::Path,
    range::Range,
    sync::{
        Arc, Mutex,
        atomic::{self, AtomicU64},
    },
    thread::{JoinHandle, sleep, spawn},
    time::Duration,
};

use cpal::traits::{DeviceTrait, StreamTrait};
use crossbeam_channel::Sender;
use ringbuf::{
    HeapRb,
    traits::{Consumer, Observer, Producer, Split},
};
use symphonia::core::{
    audio::Signal,
    errors::{Error as SymphoniaError, Result as SymphoniaResult},
    formats::{SeekMode, SeekTo, SeekedTo},
    units::Time as SymphoniaTime,
};
use tap::{Conv, Pipe};
use tracing::{error, info_span};

use crate::{
    SAMPLE_RATE,
    processing::time::{Beats, Samples, Tempo, Time, TimeSignature},
    streaming::{
        clip::{AudioClipData, Clip, ClipData, ClipTiming, ClipTimingBeats, ClipTimingSamples, SymphoniaClipData},
        track::Track,
    },
};

#[derive(Clone)]
pub struct Playlist {
    pub tracks: Vec<Track>,
    pub time_signature: TimeSignature,
    pub tempo: Tempo,
    pub preview: Option<ClipTiming>,
    pub metronome: bool
}

pub struct PlaylistOutput {
    audio_engine: JoinHandle<()>,
    pub audio_engine_tx: Sender<AudioEngineMessage>,
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

pub enum AudioEngineMessage {
    Play,
    Stop,
    Seek(Samples),
    Update(Playlist),
    UpdateTimings(HashMap<usize, ClipTiming>),
    DeleteClips(Vec<usize>),
    UpdateTempo(Tempo),
    UpdateTimeSignature(TimeSignature),
    UpdateTrackGain(usize, f32),
}

pub struct PlaylistAudio {
    out: Option<PlaylistOutput>,
    playlist: Playlist,
    playhead: Arc<AtomicU64>,
}

impl Default for PlaylistAudio {
    fn default() -> Self {
        Self::new()
    }
}

impl PlaylistAudio {
    #[must_use]
    pub fn new() -> Self {
        Self {
            out: None,
            playlist: Playlist::new(),
            playhead: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn device_out(&mut self, device: &cpal::Device, config: &cpal::StreamConfig) -> &mut PlaylistOutput {
        let (mut master_tx, master_rx) = HeapRb::new(2048 * 1024).split();
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

                let mut clip_states: Vec<Vec<Option<(usize, Vec<f32>)>>> = playlist.tracks.iter().map(|t| vec![None; t.clips.len()]).collect();

                loop {
                    while let Ok(message) = audio_engine_rx.try_recv() {
                        match message {
                            AudioEngineMessage::Play => playing = true,
                            AudioEngineMessage::Stop => playing = false,
                            AudioEngineMessage::Seek(position) => {
                                playhead.store(position.u64(), atomic::Ordering::Relaxed);
                                next = position;

                                for track_states in &mut clip_states {
                                    for state in track_states {
                                        *state = None;
                                    }
                                }
                                if let Ok(mut rx) = master_rx.lock() {
                                    rx.clear();
                                }
                            }
                            AudioEngineMessage::UpdateTimings(timings) => {
                                for track in &mut playlist.tracks {
                                    for clip in &mut track.clips {
                                        if let Some(timing) = timings.get(&clip.id) {
                                            clip.timing = *timing;
                                        }
                                    }
                                }
                            }
                            AudioEngineMessage::DeleteClips(ids) => {
                                for (t_idx, track) in playlist.tracks.iter_mut().enumerate() {
                                    let mut c_idx = 0;
                                    track.clips.retain(|c| {
                                        let keep = !ids.contains(&c.id);
                                        if keep {
                                            c_idx += 1;
                                        } else if t_idx < clip_states.len() && c_idx < clip_states[t_idx].len() {
                                            clip_states[t_idx].remove(c_idx);
                                        }
                                        keep
                                    });
                                }
                            }
                            AudioEngineMessage::UpdateTempo(tempo) => playlist.tempo = tempo,
                            AudioEngineMessage::UpdateTimeSignature(ts) => playlist.time_signature = ts,
                            AudioEngineMessage::UpdateTrackGain(track, gain) => {
                                if let Some(t) = playlist.tracks.get_mut(track) {
                                    t.gain = gain;
                                }
                            }
                            AudioEngineMessage::Update(mut new) => {
                                for track in &mut new.tracks {
                                    for clip in &mut track.clips {
                                        if let Some(old_track) = playlist.tracks.iter_mut().find(|t| t.clips.iter().any(|c| c.id == clip.id))
                                            && let Some(old_clip) = old_track.clips.iter_mut().find(|c| c.id == clip.id)
                                        {
                                            std::mem::swap(&mut clip.data, &mut old_clip.data);
                                        }
                                    }
                                }

                                let mut new_states = Vec::new();
                                for track in &new.tracks {
                                    let mut track_states = Vec::new();
                                    for clip in &track.clips {
                                        let mut found_state = None;
                                        for (t_idx, old_track) in playlist.tracks.iter().enumerate() {
                                            if let Some(c_idx) = old_track.clips.iter().position(|c| c.id == clip.id)
                                                && t_idx < clip_states.len()
                                                && c_idx < clip_states[t_idx].len()
                                            {
                                                found_state = clip_states[t_idx][c_idx].take();
                                            }
                                        }
                                        track_states.push(found_state);
                                    }
                                    new_states.push(track_states);
                                }
                                clip_states = new_states;
                                playlist = new;
                            }
                        }
                    }

                    let mut looped = false;
                    if playing && let Some(preview) = playlist.preview {
                        playhead.update(atomic::Ordering::Relaxed, atomic::Ordering::Relaxed, |p| {
                            if p >= preview.as_samples(playlist.tempo).end.u64() {
                                looped = true;
                                preview.as_samples(playlist.tempo).start.u64()
                            } else {
                                p
                            }
                        });
                    }

                    if looped && let Some(preview) = playlist.preview {
                        next = preview.as_samples(playlist.tempo).start;
                        for track_states in &mut clip_states {
                            for state in track_states {
                                *state = None;
                            }
                        }
                        if let Ok(mut rx) = master_rx.lock() {
                            rx.clear();
                        }
                    }

                    let current_playhead = playhead.load(atomic::Ordering::Relaxed);

                    if playing {
                        let buffered_frames = next.0 - current_playhead as f64;
                        let target_buffer_frames = SAMPLE_RATE * 0.2;
                        let min_chunk_frames = SAMPLE_RATE * 0.1;

                        if buffered_frames <= (target_buffer_frames - min_chunk_frames) {
                            let chunk_frames = (target_buffer_frames - buffered_frames).ceil() as usize;
                            let vacant_frames = master_tx.vacant_len() / channels as usize;
                            let chunk_frames = chunk_frames.min(vacant_frames);

                            if chunk_frames > 0 {
                                let block = ClipTimingSamples {
                                    start: next,
                                    end: next + Samples(chunk_frames as f64),
                                    offset: Samples(0.),
                                };
                                let mut buffer = vec![0.; chunk_frames * channels as usize];

                                for (t_idx, track) in playlist.tracks.iter_mut().enumerate() {
                                    for (c_idx, clip) in track.clips.iter_mut().enumerate() {
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
                                            ClipData::Audio(AudioClipData { data, channels: in_channels }) => {
                                                let in_channels = *in_channels;
                                                let max_frames = data.len() / in_channels;
                                                if source.start >= max_frames {
                                                    continue;
                                                }
                                                let source_range = source.start..source.end.clamp(0, max_frames);
                                                let out_channels = channels as usize;
                                                let dest_start = destination.start * out_channels;
                                                let mut dest_idx = dest_start;
                                                for frame in source_range {
                                                    for c in 0..out_channels {
                                                        let src_c = if c < in_channels { c } else { 0 };
                                                        if dest_idx < buffer.len() {
                                                            buffer[dest_idx] = data[frame * in_channels + src_c].mul_add(track.gain, buffer[dest_idx]);
                                                        }
                                                        dest_idx += 1;
                                                    }
                                                }
                                            }
                                            ClipData::Symphonia(data) => {
                                                if source.start as u64 >= data.decoder.codec_params().n_frames.unwrap_or(u64::MAX) {
                                                    continue;
                                                }

                                                let state = &mut clip_states[t_idx][c_idx];
                                                
                                                let mut requires_seek = true;
                                                let mut leftovers = Vec::new();

                                                if let Some((expected_start, saved_leftovers)) = state {
                                                    if *expected_start == source.start {
                                                        requires_seek = false;
                                                        leftovers = std::mem::take(saved_leftovers);
                                                    }
                                                }

                                                let mut skip_frames = 0;
                                                
                                                if requires_seek {
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
                                                    skip_frames = (error_secs * SAMPLE_RATE).round() as usize;
                                                }

                                                let mut needed_frames = source.end - source.start;
                                                let out_channels = channels as usize;
                                                let mut decoded = Vec::<f32>::with_capacity(needed_frames * out_channels);

                                                let take_leftovers_frames = needed_frames.min(leftovers.len() / out_channels);
                                                decoded.extend(leftovers.drain(0..take_leftovers_frames * out_channels));
                                                needed_frames -= take_leftovers_frames;

                                                loop {
                                                    if needed_frames == 0 {
                                                        break;
                                                    }

                                                    let packet = match data.reader.format_reader.next_packet() {
                                                        Ok(packet) => packet,
                                                        Err(SymphoniaError::IoError(error)) if error.kind() == io::ErrorKind::UnexpectedEof => break,
                                                        Err(_) => break,
                                                    };

                                                    if packet.track_id() != data.reader.format_reader.tracks()[data.track].id {
                                                        continue;
                                                    }

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

                                                    let dest_frames = dest_buf.frames();
                                                    let to_skip_frames = skip_frames.min(dest_frames);
                                                    skip_frames -= to_skip_frames;

                                                    let available_frames = dest_frames - to_skip_frames;
                                                    let take_frames = needed_frames.min(available_frames);

                                                    let in_channels = dest_buf.spec().channels.count();

                                                    for i in to_skip_frames..(to_skip_frames + take_frames) {
                                                        for c in 0..out_channels {
                                                            let src_c = if c < in_channels { c } else { 0 };
                                                            decoded.push(dest_buf.chan(src_c)[i]);
                                                        }
                                                    }
                                                    needed_frames -= take_frames;

                                                    if available_frames > take_frames {
                                                        for i in (to_skip_frames + take_frames)..dest_frames {
                                                            for c in 0..out_channels {
                                                                let src_c = if c < in_channels { c } else { 0 };
                                                                leftovers.push(dest_buf.chan(src_c)[i]);
                                                            }
                                                        }
                                                    }
                                                }

                                                let dest_start = destination.start * out_channels;
                                                let dest_len = destination.len() * out_channels;
                                                for (buffer_sample, decoded_sample) in buffer[dest_start..dest_start + dest_len].iter_mut().zip(&decoded) {
                                                    *buffer_sample = (*decoded_sample).mul_add(track.gain, *buffer_sample);
                                                }

                                                *state = Some((source.start + decoded.len() / out_channels, leftovers));
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

    #[must_use]
    pub fn playing(&self) -> bool {
        self.out.as_ref().is_some_and(|out| out.playing)
    }

    #[must_use]
    pub fn playhead(&self) -> Samples {
        Samples(self.playhead.load(atomic::Ordering::Relaxed) as f64)
    }

    pub fn seek(&self, position: Time) {
        if let Some(out) = &self.out {
            out.audio_engine_tx.send(AudioEngineMessage::Seek(position.samples(self.playlist.tempo))).unwrap();
        }
    }

    #[must_use]
    pub const fn playlist(&self) -> &Playlist {
        &self.playlist
    }

    pub fn update_playlist(&mut self, update: impl FnOnce(&mut Playlist)) {
        update(&mut self.playlist);
        if let Some(out) = &self.out {
            out.audio_engine_tx.send(AudioEngineMessage::Update(self.playlist.clone())).unwrap();
        }
    }

    pub fn move_clips(&mut self, positions: HashMap<usize, (usize, ClipTiming)>) {
        let mut to_move = Vec::new();
        for track in &mut self.playlist.tracks {
            let mut i = 0;
            while i < track.clips.len() {
                if let Some(&(new_track, new_timing)) = positions.get(&track.clips[i].id) {
                    let mut clip = track.clips.remove(i);
                    clip.timing = new_timing;
                    to_move.push((new_track, clip));
                } else {
                    i += 1;
                }
            }
        }
        for (new_track_idx, clip) in to_move {
            let dest = new_track_idx.min(self.playlist.tracks.len().saturating_sub(1));
            self.playlist.tracks[dest].clips.push(clip);
        }

        if let Some(out) = &self.out {
            out.audio_engine_tx.send(AudioEngineMessage::Update(self.playlist.clone())).unwrap();
        }
    }

    pub fn update_clip_timings(&mut self, timings: HashMap<usize, ClipTiming>) {
        for track in &mut self.playlist.tracks {
            for clip in &mut track.clips {
                if let Some(timing) = timings.get(&clip.id) {
                    clip.timing = *timing;
                }
            }
        }
        if let Some(out) = &self.out {
            out.audio_engine_tx.send(AudioEngineMessage::UpdateTimings(timings)).unwrap();
        }
    }

    pub fn delete_clips(&mut self, ids: &[usize]) {
        let ids_vec = ids.to_vec();
        for track in &mut self.playlist.tracks {
            track.clips.retain(|c| !ids.contains(&c.id));
        }
        if let Some(out) = &self.out {
            out.audio_engine_tx.send(AudioEngineMessage::DeleteClips(ids_vec)).unwrap();
        }
    }

    pub fn duplicate_clips(&mut self, ids: &[usize]) -> Vec<usize> {
        let mut new_clips = Vec::new();
        let mut new_ids = Vec::new();

        let mut min_start = f64::MAX;
        let mut max_end = 0.0;
        for track in &self.playlist.tracks {
            for clip in &track.clips {
                if ids.contains(&clip.id) {
                    let start = clip.timing.as_beats(self.playlist.tempo).start.f64();
                    let end = clip.timing.as_beats(self.playlist.tempo).end.f64();
                    if start < min_start {
                        min_start = start;
                    }
                    if end > max_end {
                        max_end = end;
                    }
                }
            }
        }

        let length = max_end - min_start;
        if length <= 0.0 {
            return new_ids;
        }

        for (t_idx, track) in self.playlist.tracks.iter().enumerate() {
            for clip in &track.clips {
                if ids.contains(&clip.id) {
                    let mut new_clip = clip.clone_with_new_id();
                    let timing = clip.timing.as_beats(self.playlist.tempo);
                    new_clip.timing = ClipTiming::Beats(ClipTimingBeats {
                        start: Beats::new(timing.start.f64() + length),
                        end: Beats::new(timing.end.f64() + length),
                        offset: timing.offset,
                    });
                    new_ids.push(new_clip.id);
                    new_clips.push((t_idx, new_clip));
                }
            }
        }

        if !new_clips.is_empty() {
            for (t_idx, clip) in new_clips {
                self.playlist.tracks[t_idx].clips.push(clip);
            }
            if let Some(out) = &self.out {
                out.audio_engine_tx.send(AudioEngineMessage::Update(self.playlist.clone())).unwrap();
            }
        }

        new_ids
    }

    pub fn delete_time_selection(&mut self, track_range: std::ops::RangeInclusive<usize>, time_range: std::ops::Range<f64>) {
        let start = Beats::new(time_range.start);
        let end = Beats::new(time_range.end);
        let mut to_delete = Vec::new();
        let mut new_clips = Vec::new();
        let mut timings_to_update = HashMap::new();

        for t_idx in track_range {
            if let Some(track) = self.playlist.tracks.get(t_idx) {
                for clip in &track.clips {
                    let timing = clip.timing.as_beats(self.playlist.tempo);
                    if timing.start.f64() >= start.f64() && timing.end.f64() <= end.f64() {
                        to_delete.push(clip.id);
                    } else if timing.start.f64() < start.f64() && timing.end.f64() > end.f64() {
                        timings_to_update.insert(
                            clip.id,
                            ClipTiming::Beats(ClipTimingBeats {
                                start: timing.start,
                                end: start,
                                offset: timing.offset,
                            }),
                        );
                        let new_clip_offset = timing.offset.f64() + (end.f64() - timing.start.f64());
                        let mut new_clip = clip.clone_with_new_id();
                        new_clip.timing = ClipTiming::Beats(ClipTimingBeats {
                            start: end,
                            end: timing.end,
                            offset: Beats::new(new_clip_offset),
                        });
                        new_clips.push((t_idx, new_clip));
                    } else if timing.start.f64() >= start.f64() && timing.start.f64() < end.f64() {
                        let new_start = end;
                        let new_offset = timing.offset.f64() + (end.f64() - timing.start.f64());
                        timings_to_update.insert(
                            clip.id,
                            ClipTiming::Beats(ClipTimingBeats {
                                start: new_start,
                                end: timing.end,
                                offset: Beats::new(new_offset),
                            }),
                        );
                    } else if timing.end.f64() > start.f64() && timing.end.f64() <= end.f64() {
                        timings_to_update.insert(
                            clip.id,
                            ClipTiming::Beats(ClipTimingBeats {
                                start: timing.start,
                                end: start,
                                offset: timing.offset,
                            }),
                        );
                    }
                }
            }
        }

        if !to_delete.is_empty() {
            self.delete_clips(&to_delete);
        }
        if !timings_to_update.is_empty() {
            self.update_clip_timings(timings_to_update);
        }
        if !new_clips.is_empty() {
            for (t_idx, clip) in new_clips {
                self.playlist.tracks[t_idx].clips.push(clip);
            }
            if let Some(out) = &self.out {
                out.audio_engine_tx.send(AudioEngineMessage::Update(self.playlist.clone())).unwrap();
            }
        }
    }

    pub fn duplicate_time_selection(&mut self, track_range: std::ops::RangeInclusive<usize>, time_range: std::ops::Range<f64>) {
        let start = Beats::new(time_range.start);
        let end = Beats::new(time_range.end);
        let length = end.f64() - start.f64();
        let mut new_clips = Vec::new();

        for t_idx in track_range {
            if let Some(track) = self.playlist.tracks.get(t_idx) {
                for clip in &track.clips {
                    let timing = clip.timing.as_beats(self.playlist.tempo);

                    let clip_start = timing.start.f64().max(start.f64());
                    let clip_end = timing.end.f64().min(end.f64());

                    if clip_start < clip_end {
                        let mut new_clip = clip.clone_with_new_id();
                        let offset_add = clip_start - timing.start.f64();

                        new_clip.timing = ClipTiming::Beats(ClipTimingBeats {
                            start: Beats::new(clip_start + length),
                            end: Beats::new(clip_end + length),
                            offset: Beats::new(timing.offset.f64() + offset_add),
                        });
                        new_clips.push((t_idx, new_clip));
                    }
                }
            }
        }

        if !new_clips.is_empty() {
            for (t_idx, clip) in new_clips {
                self.playlist.tracks[t_idx].clips.push(clip);
            }
            if let Some(out) = &self.out {
                out.audio_engine_tx.send(AudioEngineMessage::Update(self.playlist.clone())).unwrap();
            }
        }
    }

    pub fn update_track_gain(&mut self, track: usize, gain: f32) {
        self.playlist.set_track_gain(track, gain);
        if let Some(out) = &self.out {
            out.audio_engine_tx.send(AudioEngineMessage::UpdateTrackGain(track, gain)).unwrap();
        }
    }

    /// Update the tempo of the playlist and return the previous tempo.
    /// `update` receives the current tempo and should return the new tempo.
    pub fn update_tempo(&mut self, update: impl FnOnce(Tempo) -> Tempo) -> Tempo {
        let old = self.playlist.tempo;
        let new = update(old);
        self.playlist.tempo = new;
        if let Some(out) = &self.out {
            out.audio_engine_tx.send(AudioEngineMessage::UpdateTempo(new)).unwrap();
        }
        old
    }

    pub fn update_beats_per_measure(&mut self, update: impl FnOnce(u32) -> u32) -> u32 {
        let old = self.playlist.time_signature.beats_per_measure;
        let new = update(old);
        self.playlist.time_signature.beats_per_measure = new;
        if let Some(out) = &self.out {
            out.audio_engine_tx.send(AudioEngineMessage::UpdateTimeSignature(self.playlist.time_signature)).unwrap();
        }
        old
    }

    pub fn update_beat_value(&mut self, update: impl FnOnce(u32) -> u32) -> u32 {
        let old = self.playlist.time_signature.beat_value;
        let new = update(old);
        self.playlist.time_signature.beat_value = new;
        if let Some(out) = &self.out {
            out.audio_engine_tx.send(AudioEngineMessage::UpdateTimeSignature(self.playlist.time_signature)).unwrap();
        }
        old
    }
}

impl Playlist {
    #[must_use]
    pub fn new() -> Self {
        Self {
            tracks: {
                let data = Arc::from(
                    (0..SAMPLE_RATE as u32 * 2)
                        .map(|index| {
                            let frame = index / 2;
                            let time = f64::from(frame) / SAMPLE_RATE;
                            ((TAU * 440.0 * time).sin() * (-time * 4.).exp()) as f32
                        })
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                );

                vec![
                    Track {
                        clips: vec![
                            Clip::new(
                                "Sine 1".to_string(),
                                ClipData::Audio(AudioClipData { data: Arc::clone(&data), channels: 2 }),
                                ClipTiming::Beats(ClipTimingBeats {
                                    start: Beats(0.),
                                    end: Beats(1.),
                                    offset: Beats(0.),
                                }),
                            ),
                            Clip::new(
                                "Sine 2".to_string(),
                                ClipData::Audio(AudioClipData { data: Arc::clone(&data), channels: 2 }),
                                ClipTiming::Beats(ClipTimingBeats {
                                    start: Beats(2.),
                                    end: Beats(3.),
                                    offset: Beats(0.),
                                }),
                            ),
                        ],
                        gain: 1.,
                    },
                    Track {
                        clips: vec![Clip::new(
                            "Sine 3".to_string(),
                            ClipData::Audio(AudioClipData { data: Arc::clone(&data), channels: 2 }),
                            ClipTiming::Beats(ClipTimingBeats {
                                start: Beats(2.5),
                                end: Beats(3.5),
                                offset: Beats(0.),
                            }),
                        )],
                        gain: 1.,
                    },
                    Track {clips:vec![],gain:1.},
                    Track {clips:vec![],gain:1.},
                    Track {clips:vec![],gain:1.},
                    Track {clips:vec![],gain:1.},
                    Track {clips:vec![],gain:1.},
                    Track {clips:vec![],gain:1.},
                    Track {clips:vec![],gain:1.},
                    Track {clips:vec![],gain:1.},
                    Track {clips:vec![],gain:1.},
                    Track {clips:vec![],gain:1.},
                    Track {clips:vec![],gain:1.},
                    Track {clips:vec![],gain:1.},
                    
                ]
            },
            time_signature: TimeSignature::default(),
            tempo: Tempo::default(),
            preview: None,
            metronome: false
        }
    }

    pub fn set_track_gain(&mut self, index: usize, gain: f32) {
        self.tracks[index].gain = gain;
    }

    /// Add clips to the playlist from a file path.
    /// The file will be added starting at the track at the specified index, starting at the given time, then continue down subsequent tracks if the file contains multiple clips.
    /// Tracks will be added to the playlist if there are not enough tracks to fit all the clips.
    ///
    /// # Errors
    /// Returns an error if there was an issue reading the file or decoding the clips.
    pub fn add_clips(&mut self, track: usize, path: Arc<Path>, start: Time) -> SymphoniaResult<()> {
        let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let clips = SymphoniaClipData::from_path(path)?;
        self.tracks.resize((track + clips.len()).max(self.tracks.len()), Track { clips: Vec::new(), gain: 1. });
        for (track, clip) in self.tracks.iter_mut().skip(track).zip(clips) {
            let clip = clip?;
            let span = info_span!("clip_add", ?clip);
            let _enter = span.enter();
            track.clips.push(Clip::new(
                name.clone(),
                ClipData::Symphonia(clip.clone()),
                ClipTiming::Samples(ClipTimingSamples {
                    start: start.samples(self.tempo),
                    end: start.samples(self.tempo)
                        + Samples(
                            clip.decoder
                                .codec_params()
                                .pipe(|codec_params| codec_params.time_base.zip(codec_params.n_frames))
                                .map_or_else(
                                    || {
                                        error!(params = ?clip.decoder.codec_params(), "no time base or frame count on clip to calculate duration", );
                                        Duration::from_secs(5)
                                    },
                                    |(time_base, n_frames)| time_base.calc_time(n_frames).conv::<Duration>(),
                                )
                                .as_secs_f64()
                                * SAMPLE_RATE,
                        ),
                    offset: Samples(0.),
                }),
            ));
        }
        Ok(())
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