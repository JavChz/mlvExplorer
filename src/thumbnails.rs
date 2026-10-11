//! Background thumbnail extraction and texture cache.
//! Checks SQLite for cached JPEG thumbnails. If missing, spawns a background task
//! to decode frame 0, render a preview superpixel, compress to JPEG, store in DB,
//! and load into an egui::TextureHandle.

use crate::db::{load_thumb, open_connection, store_thumb};
use crate::mlv::{render_preview, FrameSource};
use crossbeam_channel::{unbounded, Receiver, Sender};
use egui::{ColorImage, Context, TextureHandle, TextureOptions};
use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::path::PathBuf;

struct ThumbResult {
    video_id: i64,
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

pub struct ThumbnailManager {
    _db_path: PathBuf,
    textures: HashMap<i64, TextureHandle>,
    in_flight: HashSet<i64>,
    tx: Sender<(i64, String)>,
    rx: Receiver<ThumbResult>,
}

impl ThumbnailManager {
    pub fn new(db_path: PathBuf, ctx: Context) -> Self {
        let (tx_req, rx_req) = unbounded::<(i64, String)>();
        let (tx_res, rx_res) = unbounded::<ThumbResult>();

        // Spawn background worker thread for thumbnail generation
        let worker_db_path = db_path.clone();
        std::thread::spawn(move || {
            let conn = open_connection(&worker_db_path).ok();
            while let Ok((video_id, file_path)) = rx_req.recv() {
                // 1. Try loading cached JPEG from DB
                if let Some(ref c) = conn {
                    if let Some(jpeg_bytes) = load_thumb(c, video_id) {
                        if let Ok(img) = image::load_from_memory(&jpeg_bytes) {
                            let rgba = img.to_rgba8();
                            let (w, h) = (rgba.width(), rgba.height());
                            let _ = tx_res.send(ThumbResult {
                                video_id,
                                width: w,
                                height: h,
                                rgba: rgba.into_raw(),
                            });
                            ctx.request_repaint();
                            continue;
                        }
                    }
                }

                // 2. Decode frame 0 from MLV
                if let Ok(source) = FrameSource::open(&file_path) {
                    if let Ok(frame) = source.decode(0) {
                        let (w, h, rgba) = render_preview(&frame, &source.clip.meta, 360);
                        
                        // Compress to JPEG for compact SQLite storage
                        if let Some(ref c) = conn {
                            let mut jpeg_buf = Vec::new();
                            let mut cursor = Cursor::new(&mut jpeg_buf);
                            let img_buf = image::RgbaImage::from_raw(w, h, rgba.clone());
                            if let Some(img) = img_buf {
                                let rgb = image::DynamicImage::ImageRgba8(img).to_rgb8();
                                if rgb.write_to(&mut cursor, image::ImageFormat::Jpeg).is_ok() {
                                    store_thumb(c, video_id, &jpeg_buf);
                                }
                            }
                        }

                        let _ = tx_res.send(ThumbResult {
                            video_id,
                            width: w,
                            height: h,
                            rgba,
                        });
                        ctx.request_repaint();
                    }
                }
            }
        });

        Self {
            _db_path: db_path,
            textures: HashMap::new(),
            in_flight: HashSet::new(),
            tx: tx_req,
            rx: rx_res,
        }
    }

    /// Process incoming thumbnail completions
    pub fn update(&mut self, ctx: &Context) {
        while let Ok(res) = self.rx.try_recv() {
            self.in_flight.remove(&res.video_id);
            if res.width > 0 && res.height > 0 && !res.rgba.is_empty() {
                let color_img = ColorImage::from_rgba_unmultiplied(
                    [res.width as usize, res.height as usize],
                    &res.rgba,
                );
                let handle = ctx.load_texture(
                    format!("thumb_{}", res.video_id),
                    color_img,
                    TextureOptions::LINEAR,
                );
                self.textures.insert(res.video_id, handle);
            }
        }
    }

    /// Request or get thumbnail texture for a video
    pub fn get_or_request(&mut self, video_id: i64, file_path: &str) -> Option<&TextureHandle> {
        if self.textures.contains_key(&video_id) {
            return self.textures.get(&video_id);
        }

        if !self.in_flight.contains(&video_id) {
            self.in_flight.insert(video_id);
            let _ = self.tx.send((video_id, file_path.to_string()));
        }

        None
    }
}
