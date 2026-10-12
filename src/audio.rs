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
    pub speed: f32,
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
            speed: 1.0,
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
                sink.set_speed(self.speed);
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

    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed.clamp(0.25, 4.0);
        if let Some(ref sink) = self.sink {
            sink.set_speed(self.speed);
        }
    }

    pub fn play(&mut self, current_time_sec: f32) {
        self.is_playing = true;
        if let Some(ref sink) = self.sink {
            if sink.is_paused() && !sink.empty() {
                let cur = self.current_playback_time().unwrap_or(self.current_time_sec);
                if (cur - current_time_sec).abs() < 0.08 {
                    sink.play();
                    return;
                }
            }
        }
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
        let mut start_sample = (self.current_time_sec * sample_rate as f32 * channels as f32) as usize;
        start_sample -= start_sample % channels;

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

    pub fn current_playback_time(&self) -> Option<f32> {
        if self.is_playing {
            self.sink.as_ref().map(|s| self.current_time_sec + s.get_pos().as_secs_f32())
        } else {
            Some(self.current_time_sec)
        }
    }

    pub fn is_finished(&self) -> bool {
        self.sink.as_ref().map(|s| s.empty()).unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_engine_load_and_info() {
        let mut engine = AudioEngine::new();
        assert!(!engine.has_audio());

        let meta = AudioMetadata {
            format: 1,
            channels: 2,
            sample_rate: 48000,
            bits_per_sample: 16,
        };
        // 1 second of stereo 48kHz audio (96000 samples)
        let samples = vec![0i16; 96000];
        engine.load(meta, samples);

        assert!(engine.has_audio());
        assert_eq!(engine.audio_info(), Some("48000Hz 16-bit Stereo".to_string()));
        assert_eq!(engine.current_playback_time(), Some(0.0));

        engine.set_volume(0.8);
        assert_eq!(engine.volume, 0.8);

        engine.set_muted(true);
        assert!(engine.is_muted);

        engine.set_speed(1.5);
        assert_eq!(engine.speed, 1.5);
    }
}
