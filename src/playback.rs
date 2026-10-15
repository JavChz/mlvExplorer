//! Multi-threaded read-ahead decoder for smooth playback & scrubbing.
//! Worker threads always decode the frames closest to the playhead first.

use crate::mlv::{DecodedFrame, FrameSource};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;

struct State {
    target: usize,
    ahead: usize,
    looping: bool,
    cache: HashMap<usize, Arc<DecodedFrame>>,
    in_flight: HashSet<usize>,
    quit: bool,
    decoded_times: Vec<Instant>,
    last_error: Option<String>,
}

struct Shared {
    state: Mutex<State>,
    cv: Condvar,
}

pub struct Playback {
    pub source: Arc<FrameSource>,
    pub audio: Arc<Mutex<crate::audio::AudioEngine>>,
    shared: Arc<Shared>,
    audio_rx: Mutex<Option<crossbeam_channel::Receiver<Option<(crate::mlv::types::AudioMetadata, Vec<i16>)>>>>,
}

impl Playback {
    pub fn from_loaded_data(
        source: Arc<FrameSource>,
        first_frame: Option<Arc<DecodedFrame>>,
        ctx: egui::Context,
    ) -> Self {
        let m = &source.clip.meta;
        let frame_bytes = (m.width as usize * m.height as usize * 2).max(1);
        // ~1.5 GB budget for read-ahead buffer so playback never starves
        let ahead = (1536 * 1024 * 1024 / frame_bytes).clamp(12, 120);

        let mut initial_cache = HashMap::new();
        if let Some(ff) = first_frame {
            initial_cache.insert(0, ff);
        }

        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                target: 0,
                ahead,
                looping: true,
                cache: initial_cache,
                in_flight: HashSet::new(),
                quit: false,
                decoded_times: Vec::new(),
                last_error: None,
            }),
            cv: Condvar::new(),
        });

        // Initialize Audio Engine on caller thread (main UI thread)
        let audio_engine = crate::audio::AudioEngine::new();
        let audio = Arc::new(Mutex::new(audio_engine));

        // Asynchronously load audio PCM on a background thread without moving AudioEngine across threads
        let (audio_tx, audio_rx) = crossbeam_channel::bounded(1);
        let source_clone = source.clone();
        let ctx_clone = ctx.clone();
        std::thread::spawn(move || {
            let res = source_clone.load_audio_pcm();
            let _ = audio_tx.send(res);
            ctx_clone.request_repaint();
        });

        let threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .clamp(4, 16);
        for _ in 0..threads {
            let shared = shared.clone();
            let source = source.clone();
            let ctx = ctx.clone();
            std::thread::spawn(move || worker(shared, source, ctx));
        }
        Self {
            source,
            audio,
            shared,
            audio_rx: Mutex::new(Some(audio_rx)),
        }
    }

    /// Poll asynchronous audio extraction and upload PCM samples into audio engine
    pub fn update_audio(&self) {
        let mut rx_guard = self.audio_rx.lock().unwrap();
        if let Some(ref rx) = *rx_guard {
            match rx.try_recv() {
                Ok(audio_opt) => {
                    if let Some((meta, samples)) = audio_opt {
                        if let Ok(mut engine) = self.audio.lock() {
                            engine.load(meta, samples);
                        }
                    }
                    *rx_guard = None; // Finished loading audio
                }
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    *rx_guard = None;
                }
                Err(crossbeam_channel::TryRecvError::Empty) => {}
            }
        }
    }

    pub fn new(source: Arc<FrameSource>, ctx: egui::Context) -> Self {
        let first_frame = source.decode(0).ok().map(Arc::new);
        Self::from_loaded_data(source, first_frame, ctx)
    }

    pub fn frame_count(&self) -> usize {
        self.source.frame_count()
    }

    pub fn set_target(&self, frame: usize, looping: bool) {
        let mut s = self.shared.state.lock().unwrap();
        if s.target != frame || s.looping != looping {
            s.target = frame;
            s.looping = looping;
            self.shared.cv.notify_all();
        }
    }

    pub fn get(&self, frame: usize) -> Option<Arc<DecodedFrame>> {
        self.shared.state.lock().unwrap().cache.get(&frame).cloned()
    }

    pub fn is_ready(&self, frame: usize) -> bool {
        self.shared.state.lock().unwrap().cache.contains_key(&frame)
    }

    /// Number of contiguous frames ready immediately ahead of the target frame
    pub fn ready_ahead(&self, start_frame: usize, count: usize) -> usize {
        let s = self.shared.state.lock().unwrap();
        let mut ready = 0;
        let total = self.source.frame_count();
        for k in 0..count {
            let f = if start_frame + k < total {
                start_frame + k
            } else if s.looping && total > 0 {
                (start_frame + k) % total
            } else {
                break;
            };
            if s.cache.contains_key(&f) {
                ready += 1;
            } else {
                break;
            }
        }
        ready
    }

    /// Total number of cached frames currently in memory
    pub fn cache_count(&self) -> usize {
        self.shared.state.lock().unwrap().cache.len()
    }

    /// Target buffer ahead depth
    pub fn ahead_budget(&self) -> usize {
        self.shared.state.lock().unwrap().ahead
    }

    /// Frames decoded per second over the last second.
    pub fn decode_fps(&self) -> f32 {
        let mut s = self.shared.state.lock().unwrap();
        let now = Instant::now();
        s.decoded_times
            .retain(|t| now.duration_since(*t).as_secs_f32() < 1.0);
        s.decoded_times.len() as f32
    }

    /// Cached frame indices (for the buffer indicator on the timeline).
    pub fn cached_frames(&self) -> Vec<usize> {
        self.shared.state.lock().unwrap().cache.keys().copied().collect()
    }

    pub fn last_error(&self) -> Option<String> {
        self.shared.state.lock().unwrap().last_error.clone()
    }
}

impl Drop for Playback {
    fn drop(&mut self) {
        let mut s = self.shared.state.lock().unwrap();
        s.quit = true;
        s.cache.clear();
        self.shared.cv.notify_all();
        drop(s);
        if let Ok(mut audio) = self.audio.lock() {
            audio.stop();
        }
    }
}

fn window(s: &State, count: usize) -> impl Iterator<Item = usize> + '_ {
    (0..s.ahead).filter_map(move |k| {
        let f = s.target + k;
        if f < count {
            Some(f)
        } else if s.looping && count > 0 {
            Some(f % count)
        } else {
            None
        }
    })
}

fn worker(shared: Arc<Shared>, source: Arc<FrameSource>, ctx: egui::Context) {
    let count = source.frame_count();
    loop {
        let job = {
            let mut s = shared.state.lock().unwrap();
            loop {
                if s.quit {
                    return;
                }
                let pick = window(&s, count)
                    .find(|f| !s.cache.contains_key(f) && !s.in_flight.contains(f));
                if let Some(f) = pick {
                    s.in_flight.insert(f);
                    // Wake up another idle worker thread so all threads decode simultaneously!
                    shared.cv.notify_one();
                    break f;
                }
                s = shared.cv.wait(s).unwrap();
            }
        };

        let result = source.decode(job);

        let mut s = shared.state.lock().unwrap();
        s.in_flight.remove(&job);
        if s.quit {
            return;
        }
        match result {
            Ok(frame) => {
                s.cache.insert(job, Arc::new(frame));
                s.decoded_times.push(Instant::now());
            }
            Err(e) => {
                s.last_error = Some(e);
                // Insert nothing; avoid hot-looping on a broken frame
                std::thread::sleep(std::time::Duration::from_millis(30));
            }
        }
        // Evict frames outside the window (keep 16 frames behind for smooth reverse / scrubbing)
        let target = s.target;
        let ahead = s.ahead;
        // If whole clip fits in ahead budget, never evict!
        if count > ahead {
            s.cache.retain(|&f, _| {
                let fwd = (f + count - target) % count.max(1);
                fwd < ahead + 4 || count - fwd <= 16
            });
        }
        // Wake another worker if there are more jobs in the window
        shared.cv.notify_one();
        drop(s);
        ctx.request_repaint();
    }
}
