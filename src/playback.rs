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
    shared: Arc<Shared>,
}

impl Playback {
    pub fn new(source: Arc<FrameSource>, ctx: egui::Context) -> Self {
        let m = &source.clip.meta;
        let frame_bytes = (m.width as usize * m.height as usize * 2).max(1);
        // ~1.5 GB budget for read-ahead buffer so playback never starves
        let ahead = (1536 * 1024 * 1024 / frame_bytes).clamp(12, 120);
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                target: 0,
                ahead,
                looping: true,
                cache: HashMap::new(),
                in_flight: HashSet::new(),
                quit: false,
                decoded_times: Vec::new(),
                last_error: None,
            }),
            cv: Condvar::new(),
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
        Self { source, shared }
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
        drop(s);
        ctx.request_repaint();
    }
}
