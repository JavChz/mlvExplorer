//! Multi-threaded pipelined read-ahead decoder for smooth playback & scrubbing.
//! Stage 1: Dedicated sequential I/O reader opens independent sequential handles and reads raw frame bytes.
//! Stage 2: Parallel CPU worker threads decode into recycled frame buffers, closest to playhead first.

use crate::mlv::{DecodedFrame, FrameSource};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Default)]
pub struct PlaybackSnapshot {
    pub is_current_ready: bool,
    pub ready_ahead: usize,
    pub cache_count: usize,
    pub ahead_budget: usize,
    pub decode_fps: f32,
    pub cached_ranges: Vec<(usize, usize)>,
    pub failed_count: usize,
    pub last_error: Option<String>,
}

struct State {
    target: usize,
    ahead: usize,
    looping: bool,
    reverse: bool,
    loop_range: Option<(usize, usize)>,
    cache: HashMap<usize, Arc<DecodedFrame>>,
    in_flight: HashSet<usize>,
    failed_attempts: HashMap<usize, u8>,
    failed_frames: HashSet<usize>,
    free_buffers: Vec<Vec<u16>>,
    quit: bool,
    decoded_times: Vec<Instant>,
    last_error: Option<String>,
    io_fatal_error: Option<String>,
    last_ui_repaint: Instant,
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
                reverse: false,
                loop_range: None,
                cache: initial_cache,
                in_flight: HashSet::new(),
                failed_attempts: HashMap::new(),
                failed_frames: HashSet::new(),
                free_buffers: Vec::new(),
                quit: false,
                decoded_times: Vec::new(),
                last_error: None,
                io_fatal_error: None,
                last_ui_repaint: Instant::now(),
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

        let decode_threads = std::thread::available_parallelism()
            .map(|n| n.get().saturating_sub(1))
            .unwrap_or(4)
            .clamp(2, 12);

        let (raw_tx, raw_rx) = crossbeam_channel::bounded::<(usize, Vec<u8>)>(decode_threads.clamp(2, 6));
        let (raw_pool_tx, raw_pool_rx) = crossbeam_channel::bounded::<Vec<u8>>(decode_threads * 2 + 2);

        {
            let shared = shared.clone();
            let source = source.clone();
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                io_reader(shared, source, raw_tx, raw_pool_rx, ctx);
            });
        }

        for _ in 0..decode_threads {
            let shared = shared.clone();
            let source = source.clone();
            let raw_rx = raw_rx.clone();
            let raw_pool_tx = raw_pool_tx.clone();
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                decode_worker(shared, source, raw_rx, raw_pool_tx, ctx);
            });
        }
        // Drop coordinator's rx so channel disconnect immediately wakes io_reader when workers exit
        drop(raw_rx);

        Self {
            source,
            audio,
            shared,
            audio_rx: Mutex::new(Some(audio_rx)),
        }
    }

    /// Poll asynchronous audio extraction and upload PCM samples into audio engine.
    /// Returns true if audio was freshly loaded on this call.
    pub fn update_audio(&self) -> bool {
        let mut rx_guard = self.audio_rx.lock().unwrap();
        if let Some(ref rx) = *rx_guard {
            match rx.try_recv() {
                Ok(audio_opt) => {
                    let mut loaded = false;
                    if let Some((meta, samples)) = audio_opt {
                        if let Ok(mut engine) = self.audio.lock() {
                            engine.load(meta, samples);
                            loaded = true;
                        }
                    }
                    *rx_guard = None; // Finished loading audio
                    return loaded;
                }
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    *rx_guard = None;
                }
                Err(crossbeam_channel::TryRecvError::Empty) => {}
            }
        }
        false
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

    pub fn set_loop_range(&self, loop_range: Option<(usize, usize)>) {
        let mut s = self.shared.state.lock().unwrap();
        if s.loop_range != loop_range {
            s.loop_range = loop_range;
            self.shared.cv.notify_all();
        }
    }

    pub fn set_reverse(&self, reverse: bool) {
        let mut s = self.shared.state.lock().unwrap();
        if s.reverse != reverse {
            s.reverse = reverse;
            self.shared.cv.notify_all();
        }
    }

    pub fn get(&self, frame: usize) -> Option<Arc<DecodedFrame>> {
        self.shared.state.lock().unwrap().cache.get(&frame).cloned()
    }

    pub fn is_ready(&self, frame: usize) -> bool {
        self.shared.state.lock().unwrap().cache.contains_key(&frame)
    }

    pub fn is_failed(&self, frame: usize) -> bool {
        self.shared.state.lock().unwrap().failed_frames.contains(&frame)
    }

    pub fn is_ready_or_failed(&self, frame: usize) -> bool {
        let s = self.shared.state.lock().unwrap();
        s.cache.contains_key(&frame) || s.failed_frames.contains(&frame)
    }

    /// Fetch UI playback stats and merged cached ranges in one single mutex lock.
    pub fn snapshot(&self, current_frame: usize) -> PlaybackSnapshot {
        let mut s = self.shared.state.lock().unwrap();
        let total = self.source.frame_count();
        let is_current_ready = s.cache.contains_key(&current_frame);

        let mut ready_ahead = 0usize;
        if total > 0 {
            if let Some((in_f, out_f)) = s.loop_range.filter(|&(a, b)| a <= b && b < total && s.looping) {
                let span = out_f - in_f + 1;
                let t = current_frame.clamp(in_f, out_f);
                for k in 0..24.min(span) {
                    let f = if s.reverse {
                        in_f + ((t - in_f) + span - (k % span)) % span
                    } else {
                        in_f + ((t - in_f) + k) % span
                    };
                    if s.cache.contains_key(&f) {
                        ready_ahead += 1;
                    } else if s.failed_frames.contains(&f) {
                        continue;
                    } else {
                        break;
                    }
                }
            } else {
                for k in 0..24 {
                    let f = if s.reverse {
                        if current_frame >= k {
                            current_frame - k
                        } else if s.looping {
                            total - ((k - current_frame - 1) % total + 1)
                        } else {
                            break;
                        }
                    } else if current_frame + k < total {
                        current_frame + k
                    } else if s.looping {
                        (current_frame + k) % total
                    } else {
                        break;
                    };
                    if s.cache.contains_key(&f) {
                        ready_ahead += 1;
                    } else if s.failed_frames.contains(&f) {
                        continue;
                    } else {
                        break;
                    }
                }
            }
        }

        let cache_count = s.cache.len();
        let ahead_budget = s.ahead;
        let failed_count = s.failed_frames.len();
        let last_error = s.io_fatal_error.clone().or_else(|| s.last_error.clone());

        let now = Instant::now();
        s.decoded_times
            .retain(|t| now.duration_since(*t).as_secs_f32() < 1.0);
        let decode_fps = s.decoded_times.len() as f32;

        let cached_ranges = merge_sorted_ranges(s.cache.keys().copied().collect());

        PlaybackSnapshot {
            is_current_ready,
            ready_ahead,
            cache_count,
            ahead_budget,
            decode_fps,
            cached_ranges,
            failed_count,
            last_error,
        }
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

    pub fn fatal_error(&self) -> Option<String> {
        self.shared.state.lock().unwrap().io_fatal_error.clone()
    }
}

impl Drop for Playback {
    fn drop(&mut self) {
        let mut s = self.shared.state.lock().unwrap();
        s.quit = true;
        s.cache.clear();
        s.free_buffers.clear();
        self.shared.cv.notify_all();
        drop(s);
        if let Ok(mut audio) = self.audio.lock() {
            audio.stop();
        }
    }
}

pub fn merge_sorted_ranges(mut keys: Vec<usize>) -> Vec<(usize, usize)> {
    if keys.is_empty() {
        return Vec::new();
    }
    keys.sort_unstable();
    let mut ranges = Vec::new();
    let mut start = keys[0];
    let mut end = start;
    for &f in &keys[1..] {
        if f <= end + 1 {
            end = f;
        } else {
            ranges.push((start, end));
            start = f;
            end = f;
        }
    }
    ranges.push((start, end));
    ranges
}

pub fn compute_window_indices(
    target: usize,
    ahead: usize,
    looping: bool,
    loop_range: Option<(usize, usize)>,
    count: usize,
) -> Vec<usize> {
    compute_window_indices_dir(target, ahead, looping, loop_range, count, false)
}

pub fn compute_window_indices_dir(
    target: usize,
    ahead: usize,
    looping: bool,
    loop_range: Option<(usize, usize)>,
    count: usize,
    reverse: bool,
) -> Vec<usize> {
    if count == 0 || ahead == 0 {
        return Vec::new();
    }
    if let Some((in_f, out_f)) = loop_range.filter(|&(a, b)| a <= b && b < count && looping) {
        let span = out_f - in_f + 1;
        let limit = ahead.min(span);
        let t = target.clamp(in_f, out_f);
        let mut out = Vec::with_capacity(limit);
        for k in 0..limit {
            let f = if reverse {
                in_f + ((t - in_f) + span - (k % span)) % span
            } else {
                in_f + ((t - in_f) + k) % span
            };
            out.push(f);
        }
        out
    } else {
        let t = target.min(count - 1);
        if reverse {
            let limit = if looping {
                ahead.min(count)
            } else {
                ahead.min(t + 1)
            };
            let mut out = Vec::with_capacity(limit);
            for k in 0..limit {
                let f = if t >= k {
                    t - k
                } else {
                    count - ((k - t - 1) % count + 1)
                };
                out.push(f);
            }
            out
        } else {
            let limit = if looping {
                ahead.min(count)
            } else {
                ahead.min(count - t)
            };
            let mut out = Vec::with_capacity(limit);
            for k in 0..limit {
                let f = t + k;
                out.push(if f < count { f } else { f % count });
            }
            out
        }
    }
}

fn is_frame_retained(s: &State, f: usize, count: usize) -> bool {
    if count == 0 {
        return false;
    }
    if count <= s.ahead {
        return true;
    }
    if let Some((a, b)) = s.loop_range.filter(|&(a, b)| a <= b && b < count && s.looping) {
        if (b - a + 1) <= s.ahead && f >= a && f <= b {
            return true;
        }
    }
    let t = s.target.min(count - 1);
    if s.reverse {
        let bwd = (t + count - f) % count;
        bwd < s.ahead + 4 || count - bwd <= 16
    } else {
        let fwd = (f + count - t) % count;
        fwd < s.ahead + 4 || count - fwd <= 16
    }
}

fn should_decode_frame(s: &State, f: usize, count: usize) -> bool {
    if count == 0 {
        return false;
    }
    if count <= s.ahead {
        return true;
    }
    if let Some((a, b)) = s.loop_range.filter(|&(a, b)| a <= b && b < count && s.looping) {
        if (b - a + 1) <= s.ahead && f >= a && f <= b {
            return true;
        }
    }
    let t = s.target.min(count - 1);
    if s.reverse {
        let bwd = (t + count - f) % count;
        bwd < s.ahead + 4
    } else {
        let fwd = (f + count - t) % count;
        fwd < s.ahead + 4
    }
}

fn io_reader(
    shared: Arc<Shared>,
    source: Arc<FrameSource>,
    raw_tx: crossbeam_channel::Sender<(usize, Vec<u8>)>,
    raw_pool_rx: crossbeam_channel::Receiver<Vec<u8>>,
    ctx: egui::Context,
) {
    let count = source.frame_count();
    if count == 0 {
        return;
    }
    let files = match source.open_chunk_files() {
        Ok(f) => f,
        Err(e) => {
            let mut s = shared.state.lock().unwrap();
            s.io_fatal_error = Some(e.clone());
            s.last_error = Some(e);
            shared.cv.notify_all();
            ctx.request_repaint();
            return;
        }
    };

    loop {
        let job = {
            let mut s = shared.state.lock().unwrap();
            loop {
                if s.quit {
                    return;
                }
                let win = compute_window_indices_dir(
                    s.target,
                    s.ahead,
                    s.looping,
                    s.loop_range,
                    count,
                    s.reverse,
                );
                let pick = win.into_iter().find(|f| {
                    !s.cache.contains_key(f)
                        && !s.in_flight.contains(f)
                        && !s.failed_frames.contains(f)
                });
                if let Some(f) = pick {
                    s.in_flight.insert(f);
                    break f;
                }
                s = shared.cv.wait(s).unwrap();
            }
        };

        let mut packed = raw_pool_rx.try_recv().unwrap_or_default();
        match source.read_raw_frame_into(&files, job, &mut packed) {
            Ok(()) => {
                if raw_tx.send((job, packed)).is_err() {
                    let mut s = shared.state.lock().unwrap();
                    s.in_flight.remove(&job);
                    shared.cv.notify_all();
                    return;
                }
            }
            Err(e) => {
                let mut s = shared.state.lock().unwrap();
                s.in_flight.remove(&job);
                let attempts = {
                    let cnt = s.failed_attempts.entry(job).or_insert(0);
                    *cnt = cnt.saturating_add(1);
                    *cnt
                };
                if attempts >= 2 || e.contains("out of range") {
                    s.failed_frames.insert(job);
                }
                s.last_error = Some(e);
                shared.cv.notify_all();
                drop(s);
                ctx.request_repaint();
                std::thread::sleep(Duration::from_millis(30));
            }
        }
    }
}

fn decode_worker(
    shared: Arc<Shared>,
    source: Arc<FrameSource>,
    raw_rx: crossbeam_channel::Receiver<(usize, Vec<u8>)>,
    raw_pool_tx: crossbeam_channel::Sender<Vec<u8>>,
    ctx: egui::Context,
) {
    let count = source.frame_count();
    while let Ok((job, packed)) = raw_rx.recv() {
        let (decode_it, buf) = {
            let mut s = shared.state.lock().unwrap();
            if s.quit {
                s.in_flight.remove(&job);
                return;
            }
            let want = should_decode_frame(&s, job, count);
            let buf = if want {
                s.free_buffers.pop().unwrap_or_default()
            } else {
                Vec::new()
            };
            (want, buf)
        };

        if !decode_it {
            let _ = raw_pool_tx.try_send(packed);
            let mut s = shared.state.lock().unwrap();
            s.in_flight.remove(&job);
            shared.cv.notify_all();
            continue;
        }

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            source.decode_packed(job, &packed, buf, false)
        }))
        .unwrap_or_else(|_| Err(format!("Decoder panic on frame {}", job)));
        let _ = raw_pool_tx.try_send(packed);

        let (should_repaint, is_err) = {
            let mut s = shared.state.lock().unwrap();
            s.in_flight.remove(&job);
            if s.quit {
                return;
            }
            let is_err = match result {
                Ok(frame) => {
                    s.cache.insert(job, Arc::new(frame));
                    s.decoded_times.push(Instant::now());
                    false
                }
                Err(e) => {
                    let attempts = {
                        let cnt = s.failed_attempts.entry(job).or_insert(0);
                        *cnt = cnt.saturating_add(1);
                        *cnt
                    };
                    if attempts >= 2
                        || e.starts_with("LJ92:")
                        || e.starts_with("Decoder panic")
                    {
                        s.failed_frames.insert(job);
                    }
                    s.last_error = Some(e);
                    true
                }
            };

            if !is_err && count > s.ahead {
                let mut to_evict = Vec::new();
                for &f in s.cache.keys() {
                    if !is_frame_retained(&s, f, count) {
                        to_evict.push(f);
                    }
                }
                for f in to_evict {
                    if let Some(arc) = s.cache.remove(&f) {
                        if let Ok(df) = Arc::try_unwrap(arc) {
                            if s.free_buffers.len() < 20 {
                                s.free_buffers.push(df.data);
                            }
                        }
                    }
                }
            }

            shared.cv.notify_all();

            let is_urgent = job == s.target || job == s.target + 1 || job + 1 == s.target;
            let throttle_ok = is_err || s.last_ui_repaint.elapsed() >= Duration::from_millis(100);
            let repaint = if is_urgent || throttle_ok {
                s.last_ui_repaint = Instant::now();
                true
            } else {
                false
            };
            (repaint, is_err)
        };

        if should_repaint {
            ctx.request_repaint();
        }
        if is_err {
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_sorted_ranges() {
        assert_eq!(merge_sorted_ranges(vec![]), vec![]);
        assert_eq!(
            merge_sorted_ranges(vec![5, 1, 2, 3, 7, 8, 12]),
            vec![(1, 3), (5, 5), (7, 8), (12, 12)]
        );
    }

    #[test]
    fn test_compute_window_indices_with_loop_range() {
        // Normal wrapping around clip end
        assert_eq!(
            compute_window_indices(8, 5, true, None, 10),
            vec![8, 9, 0, 1, 2]
        );
        // Non-looping stops at clip end
        assert_eq!(
            compute_window_indices(8, 5, false, None, 10),
            vec![8, 9]
        );
        // In/Out loop range wraps strictly inside [20..=23]
        assert_eq!(
            compute_window_indices(22, 6, true, Some((20, 23)), 100),
            vec![22, 23, 20, 21]
        );
    }

    #[test]
    fn test_compute_window_indices_reverse() {
        assert_eq!(
            compute_window_indices_dir(2, 5, true, None, 10, true),
            vec![2, 1, 0, 9, 8]
        );
        assert_eq!(
            compute_window_indices_dir(2, 5, false, None, 10, true),
            vec![2, 1, 0]
        );
        assert_eq!(
            compute_window_indices_dir(21, 6, true, Some((20, 23)), 100, true),
            vec![21, 20, 23, 22]
        );
    }

    #[test]
    fn test_should_decode_frame_skips_stale_behind_target() {
        let mut s = State {
            target: 50,
            ahead: 30,
            looping: true,
            reverse: false,
            loop_range: None,
            cache: HashMap::new(),
            in_flight: HashSet::new(),
            failed_attempts: HashMap::new(),
            failed_frames: HashSet::new(),
            free_buffers: Vec::new(),
            quit: false,
            decoded_times: Vec::new(),
            last_error: None,
            io_fatal_error: None,
            last_ui_repaint: Instant::now(),
        };

        // 5 frames behind forward target: retained in cache, but skipped by decode workers
        assert!(is_frame_retained(&s, 45, 200));
        assert!(!should_decode_frame(&s, 45, 200));
        // Ahead of target: retained AND decoded
        assert!(is_frame_retained(&s, 55, 200));
        assert!(should_decode_frame(&s, 55, 200));

        // Flip to reverse playback
        s.reverse = true;
        assert!(is_frame_retained(&s, 55, 200));
        assert!(!should_decode_frame(&s, 55, 200));
        assert!(is_frame_retained(&s, 45, 200));
        assert!(should_decode_frame(&s, 45, 200));
    }
}
