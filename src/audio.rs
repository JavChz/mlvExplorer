//! High-performance Audio Engine for Magic Lantern RAW Video playback.
//!
//! Synchronizes audio PCM samples with video frame playback, scrubbing,
//! volume adjustment, and muting using rodio.

use crate::mlv::AudioMetadata;
use rodio::{buffer::SamplesBuffer, OutputStream, OutputStreamHandle, Sink};
use std::sync::Arc;

pub struct AudioEngine {
    _stream: Option<OutputStream>,
    stream_handle: Option<OutputStreamHandle>,
    sink: Option<Sink>,
    meta: Option<AudioMetadata>,
    samples: Arc<Vec<i16>>,
    pub volume: f32,
    pub is_muted: bool,
    is_playing: bool,
    current_time_sec: f32,
}

impl AudioEngine {
    pub fn new() -> Self {
        let (stream, stream_handle) = match OutputStream::try_default() {
            Ok((s, h)) => (Some(s), Some(h)),
            Err(e) => {
                eprintln!("[Audio] Warning: Could not initialize default audio output device: {e}");
                (None, None)
            }
        };

        Self {
            _stream: stream,
            stream_handle,
            sink: None,
            meta: None,
            samples: Arc::new(Vec::new()),
            volume: 1.0,
            is_muted: false,
            is_playing: false,
            current_time_sec: 0.0,
        }
    }

    pub fn load(&mut self, meta: AudioMetadata, samples: Vec<i16>) {
        self.stop();
        self.meta = Some(meta);
        self.samples = Arc::new(samples);
        self.init_sink();
    }

    pub fn clear(&mut self) {
        self.stop();
        self.meta = None;
        self.samples = Arc::new(Vec::new());
        self.sink = None;
    }

    pub fn has_audio(&self) -> bool {
        self.meta.is_some() && !self.samples.is_empty()
    }

    pub fn audio_info(&self) -> Option<String> {
        self.meta.as_ref().map(|m| {
            let ch = if m.channels == 1 { "Mono" } else { "Stereo" };
            format!("{}Hz {}-bit {}", m.sample_rate, m.bits_per_sample, ch)
        })
    }

    fn init_sink(&mut self) {
        if let Some(ref handle) = self.stream_handle {
            if let Ok(sink) = Sink::try_new(handle) {
                sink.set_volume(if self.is_muted { 0.0 } else { self.volume });
                self.sink = Some(sink);
            }
        }
    }

    pub fn set_volume(&mut self, vol: f32) {
        self.volume = vol.clamp(0.0, 1.0);
        if let Some(ref sink) = self.sink {
            sink.set_volume(if self.is_muted { 0.0 } else { self.volume });
        }
    }

    pub fn set_muted(&mut self, muted: bool) {
        self.is_muted = muted;
        if let Some(ref sink) = self.sink {
            sink.set_volume(if self.is_muted { 0.0 } else { self.volume });
        }
    }

    pub fn play(&mut self, current_time_sec: f32) {
        self.is_playing = true;
        self.seek(current_time_sec, true);
    }

    pub fn pause(&mut self) {
        self.is_playing = false;
        if let Some(ref sink) = self.sink {
            sink.pause();
        }
    }

    pub fn stop(&mut self) {
        self.is_playing = false;
        if let Some(ref sink) = self.sink {
            sink.stop();
        }
    }

    pub fn seek(&mut self, time_sec: f32, continue_playing: bool) {
        self.current_time_sec = time_sec.max(0.0);
        let Some(meta) = self.meta.clone() else { return };
        if self.samples.is_empty() { return; }

        self.init_sink();
        let Some(ref sink) = self.sink else { return };

        let channels = meta.channels.max(1) as usize;
        let sample_rate = meta.sample_rate.max(8000);
        let start_sample = (self.current_time_sec * sample_rate as f32 * channels as f32) as usize;

        if start_sample < self.samples.len() {
            let slice = &self.samples[start_sample..];
            let source = SamplesBuffer::new(meta.channels, meta.sample_rate, slice);
            sink.append(source);
            if continue_playing {
                sink.play();
            } else {
                sink.pause();
            }
        }
    }
}
