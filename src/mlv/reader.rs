use super::types::*;
use byteorder::{LittleEndian, ReadBytesExt};
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

pub struct MlvReader {
    pub metadata: MlvMetadata,
    pub frame_index: Vec<FrameIndexEntry>,
}

impl MlvReader {
    /// Fast scan of an MLV file to extract metadata and frame offsets
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let path_ref = path.as_ref();
        let file = File::open(path_ref).map_err(|e| format!("Cannot open file {}: {}", path_ref.display(), e))?;
        let file_size = file.metadata().map_err(|e| e.to_string())?.len();
        let mut reader = BufReader::with_capacity(256 * 1024, file);

        let mut metadata = MlvMetadata {
            file_path: path_ref.to_string_lossy().to_string(),
            file_size,
            ..Default::default()
        };

        // First block must be MLVI
        let first_header = Self::read_block_header(&mut reader)?;
        if first_header.block_type != BLOCK_MLVI {
            return Err("Not a valid MLV file: Missing MLVI header".to_string());
        }

        // Parse MLVI block body
        let guid = reader.read_u64::<LittleEndian>().unwrap_or(0);
        let file_num = reader.read_u16::<LittleEndian>().unwrap_or(0);
        let file_count = reader.read_u16::<LittleEndian>().unwrap_or(0);
        let _video_class = reader.read_u16::<LittleEndian>().unwrap_or(0);
        let _audio_class = reader.read_u16::<LittleEndian>().unwrap_or(0);
        let video_frame_count = reader.read_u32::<LittleEndian>().unwrap_or(0);
        let _audio_frame_count = reader.read_u32::<LittleEndian>().unwrap_or(0);
        let fps_num = reader.read_u32::<LittleEndian>().unwrap_or(24000);
        let fps_den = reader.read_u32::<LittleEndian>().unwrap_or(1000);

        metadata.video_guid = guid;
        metadata.file_num = file_num;
        metadata.file_count = file_count;
        metadata.fps_num = fps_num;
        metadata.fps_den = fps_den;
        metadata.frame_count = video_frame_count;

        // Jump to end of MLVI block
        let mlvi_payload_read = 8 + 2 + 2 + 2 + 2 + 4 + 4 + 4 + 4; // 32 bytes
        let remaining_mlvi = first_header.block_size.saturating_sub(16 + mlvi_payload_read);
        if remaining_mlvi > 0 {
            reader.seek(SeekFrom::Current(remaining_mlvi as i64)).map_err(|e| e.to_string())?;
        }

        let mut frame_index = Vec::new();
        let mut current_offset = first_header.block_size as u64;

        // Iterate through subsequent blocks
        while current_offset < file_size {
            let header = match Self::read_block_header(&mut reader) {
                Ok(h) => h,
                Err(_) => break, // EOF or truncated block
            };

            if header.block_size < 16 {
                break; // Corrupted block size
            }

            let payload_size = header.block_size - 16;
            let block_start_offset = current_offset;

            match header.block_type {
                BLOCK_RAWI => {
                    let _x_res = reader.read_u16::<LittleEndian>().unwrap_or(0);
                    let _y_res = reader.read_u16::<LittleEndian>().unwrap_or(0);
                    let _raw_info_ver = reader.read_u32::<LittleEndian>().unwrap_or(0);
                    let _buffer = reader.read_u32::<LittleEndian>().unwrap_or(0);
                    let width = reader.read_u16::<LittleEndian>().unwrap_or(0);
                    let height = reader.read_u16::<LittleEndian>().unwrap_or(0);
                    let _pitch = reader.read_u32::<LittleEndian>().unwrap_or(0);
                    let _frame_size = reader.read_u32::<LittleEndian>().unwrap_or(0);
                    let bits_per_pixel = reader.read_u32::<LittleEndian>().unwrap_or(14);
                    let black_level = reader.read_u32::<LittleEndian>().unwrap_or(2048);
                    let white_level = reader.read_u32::<LittleEndian>().unwrap_or(15000);
                    let _dng_cfa = reader.read_u32::<LittleEndian>().unwrap_or(0);
                    let cfa_pattern = reader.read_u32::<LittleEndian>().unwrap_or(0); // 0 = RGGB

                    metadata.width = width as u32;
                    metadata.height = height as u32;
                    metadata.raw_width = width as u32;
                    metadata.raw_height = height as u32;
                    metadata.bits_per_pixel = bits_per_pixel as u16;
                    metadata.black_level = black_level;
                    metadata.white_level = white_level;
                    metadata.cfa_pattern = cfa_pattern;

                    // Seek to next block
                    let read_bytes = 2 + 2 + 4 + 4 + 2 + 2 + 4 + 4 + 4 + 4 + 4 + 4 + 4;
                    let remaining = payload_size.saturating_sub(read_bytes);
                    reader.seek(SeekFrom::Current(remaining as i64)).map_err(|e| e.to_string())?;
                }
                BLOCK_VIDF => {
                    let frame_number = reader.read_u32::<LittleEndian>().unwrap_or(frame_index.len() as u32);
                    let _crop_pos_x = reader.read_u16::<LittleEndian>().unwrap_or(0);
                    let _crop_pos_y = reader.read_u16::<LittleEndian>().unwrap_or(0);
                    let _pan_pos_x = reader.read_u16::<LittleEndian>().unwrap_or(0);
                    let _pan_pos_y = reader.read_u16::<LittleEndian>().unwrap_or(0);
                    let frame_space = reader.read_u32::<LittleEndian>().unwrap_or(0);

                    // Payload data starts after header (16) + vidf fields (16) + frameSpace
                    let vidf_hdr_size = 16 + 16 + frame_space;
                    let frame_data_offset = block_start_offset + vidf_hdr_size as u64;
                    let frame_data_size = header.block_size.saturating_sub(vidf_hdr_size);

                    frame_index.push(FrameIndexEntry {
                        frame_number,
                        file_offset: frame_data_offset,
                        payload_size: frame_data_size,
                        timestamp: header.timestamp,
                    });

                    // Skip remaining frame payload to get fast indexing
                    let read_so_far = 4 + 2 + 2 + 2 + 2 + 4;
                    let to_skip = payload_size.saturating_sub(read_so_far);
                    reader.seek(SeekFrom::Current(to_skip as i64)).map_err(|e| e.to_string())?;
                }
                BLOCK_LENS => {
                    let focal_length = reader.read_u16::<LittleEndian>().unwrap_or(0);
                    let _focal_dist = reader.read_u16::<LittleEndian>().unwrap_or(0);
                    let aperture = reader.read_u16::<LittleEndian>().unwrap_or(0);
                    let _stabilizer = reader.read_u8().unwrap_or(0);
                    let _autofocus = reader.read_u8().unwrap_or(0);
                    let _flags = reader.read_u16::<LittleEndian>().unwrap_or(0);
                    let _lens_id = reader.read_u32::<LittleEndian>().unwrap_or(0);

                    let mut lens_name_bytes = [0u8; 32];
                    let _ = reader.read_exact(&mut lens_name_bytes);
                    let lens_name = String::from_utf8_lossy(&lens_name_bytes)
                        .trim_matches(char::from(0))
                        .trim()
                        .to_string();

                    metadata.focal_length_mm = focal_length;
                    metadata.aperture_fstop = if aperture > 0 { aperture as f32 / 100.0 } else { 0.0 };
                    if !lens_name.is_empty() {
                        metadata.lens_name = lens_name;
                    }

                    let read_so_far = 2 + 2 + 2 + 1 + 1 + 2 + 4 + 32;
                    let to_skip = payload_size.saturating_sub(read_so_far);
                    if to_skip > 0 {
                        reader.seek(SeekFrom::Current(to_skip as i64)).map_err(|e| e.to_string())?;
                    }
                }
                BLOCK_EXPO => {
                    let _iso_mode = reader.read_u32::<LittleEndian>().unwrap_or(0);
                    let iso = reader.read_u32::<LittleEndian>().unwrap_or(0);
                    let shutter = reader.read_u64::<LittleEndian>().unwrap_or(0);

                    metadata.iso = iso;
                    metadata.shutter_speed_microseconds = shutter as u32;

                    let read_so_far = 4 + 4 + 8;
                    let to_skip = payload_size.saturating_sub(read_so_far);
                    if to_skip > 0 {
                        reader.seek(SeekFrom::Current(to_skip as i64)).map_err(|e| e.to_string())?;
                    }
                }
                BLOCK_IDNT => {
                    let mut cam_name_bytes = [0u8; 32];
                    let _ = reader.read_exact(&mut cam_name_bytes);
                    let mut cam_serial_bytes = [0u8; 32];
                    let _ = reader.read_exact(&mut cam_serial_bytes);

                    let cam_name = String::from_utf8_lossy(&cam_name_bytes)
                        .trim_matches(char::from(0))
                        .trim()
                        .to_string();
                    let cam_serial = String::from_utf8_lossy(&cam_serial_bytes)
                        .trim_matches(char::from(0))
                        .trim()
                        .to_string();

                    if !cam_name.is_empty() {
                        metadata.camera_name = cam_name;
                    }
                    if !cam_serial.is_empty() {
                        metadata.camera_serial = cam_serial;
                    }

                    let to_skip = payload_size.saturating_sub(64);
                    if to_skip > 0 {
                        reader.seek(SeekFrom::Current(to_skip as i64)).map_err(|e| e.to_string())?;
                    }
                }
                BLOCK_WBAL => {
                    let _wb_mode = reader.read_u32::<LittleEndian>().unwrap_or(0);
                    let kelvin = reader.read_u32::<LittleEndian>().unwrap_or(5500);
                    metadata.white_balance_kelvin = kelvin;

                    let to_skip = payload_size.saturating_sub(8);
                    if to_skip > 0 {
                        reader.seek(SeekFrom::Current(to_skip as i64)).map_err(|e| e.to_string())?;
                    }
                }
                BLOCK_RTCI => {
                    let sec = reader.read_u32::<LittleEndian>().unwrap_or(0);
                    let min = reader.read_u32::<LittleEndian>().unwrap_or(0);
                    let hour = reader.read_u32::<LittleEndian>().unwrap_or(0);
                    let mday = reader.read_u32::<LittleEndian>().unwrap_or(1);
                    let mon = reader.read_u32::<LittleEndian>().unwrap_or(0) + 1;
                    let year = reader.read_u32::<LittleEndian>().unwrap_or(100) + 1900;

                    metadata.date_recorded = Some(format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", year, mon, mday, hour, min, sec));

                    let to_skip = payload_size.saturating_sub(24);
                    if to_skip > 0 {
                        reader.seek(SeekFrom::Current(to_skip as i64)).map_err(|e| e.to_string())?;
                    }
                }
                _ => {
                    // Unknown or unneeded block; skip payload
                    reader.seek(SeekFrom::Current(payload_size as i64)).map_err(|e| e.to_string())?;
                }
            }

            current_offset += header.block_size as u64;
        }

        // If video_frame_count in MLVI was 0, use the indexed count
        if metadata.frame_count == 0 || metadata.frame_count != frame_index.len() as u32 {
            metadata.frame_count = frame_index.len() as u32;
        }

        // Default fallbacks if camera/lens were not in header
        if metadata.camera_name.is_empty() {
            metadata.camera_name = "Canon EOS RAW".to_string();
        }
        if metadata.lens_name.is_empty() {
            metadata.lens_name = "Standard Lens".to_string();
        }

        Ok(Self {
            metadata,
            frame_index,
        })
    }

    /// Read single block header (16 bytes)
    fn read_block_header<R: Read>(reader: &mut R) -> Result<MlvBlockHeader, String> {
        let mut block_type = [0u8; 4];
        reader.read_exact(&mut block_type).map_err(|e| e.to_string())?;
        let block_size = reader.read_u32::<LittleEndian>().map_err(|e| e.to_string())?;
        let timestamp = reader.read_u64::<LittleEndian>().map_err(|e| e.to_string())?;

        Ok(MlvBlockHeader {
            block_type,
            block_size,
            timestamp,
        })
    }

    /// Read raw frame bytes at the given frame index
    pub fn read_frame_bytes(&self, frame_idx: usize) -> Result<Vec<u8>, String> {
        if frame_idx >= self.frame_index.len() {
            return Err("Frame index out of bounds".to_string());
        }

        let entry = &self.frame_index[frame_idx];
        let mut file = File::open(&self.metadata.file_path).map_err(|e| e.to_string())?;
        file.seek(SeekFrom::Start(entry.file_offset)).map_err(|e| e.to_string())?;

        let mut buffer = vec![0u8; entry.payload_size as usize];
        file.read_exact(&mut buffer).map_err(|e| e.to_string())?;

        Ok(buffer)
    }
}
