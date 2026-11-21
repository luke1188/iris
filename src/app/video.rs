//! Looping H.264 MP4 background. Audio is ignored (this is a music visualizer).

use anyhow::{bail, Context, Result};
use egui::{ColorImage, TextureHandle, TextureOptions};
use image::imageops::FilterType;
use image::{DynamicImage, RgbImage};
use openh264::decoder::{DecodeOptions, Decoder, DecoderConfig, Flush};
use openh264::formats::YUVSource;
use openh264::OpenH264API;
use parking_lot::Mutex;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use super::media::SAFE_MAX_SIDE;

const VIDEO_MAX_SIDE: u32 = 1920;
const FIRST_FRAME_TIMEOUT: Duration = Duration::from_secs(4);

struct SharedFrame {
    seq: u64,
    size: [usize; 2],
    rgb: Vec<u8>,
}

struct SharedState {
    frame: Mutex<SharedFrame>,
    error: Mutex<Option<String>>,
    stop: AtomicBool,
}

pub struct VideoBackground {
    #[allow(dead_code)]
    pub path: PathBuf,
    texture: TextureHandle,
    size: [usize; 2],
    shared: Arc<SharedState>,
    last_seq: u64,
    thread: Option<JoinHandle<()>>,
}

impl VideoBackground {
    pub fn size(&self) -> [usize; 2] {
        self.size
    }

    pub fn texture(&self) -> &TextureHandle {
        &self.texture
    }

    /// Upload a newly decoded frame if the decoder thread published one.
    pub fn sync(&mut self) {
        let (seq, size, rgb) = {
            let g = self.shared.frame.lock();
            if g.seq == self.last_seq || g.rgb.is_empty() {
                return;
            }
            (g.seq, g.size, g.rgb.clone())
        };
        self.last_seq = seq;
        self.size = size;
        let color = ColorImage::from_rgb(size, &rgb);
        self.texture.set(color, TextureOptions::LINEAR);
    }
}

impl Drop for VideoBackground {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

pub fn is_video_path(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("mp4" | "m4v" | "mov")
    )
}

pub fn load_video_background(ctx: &egui::Context, path: &Path) -> Result<VideoBackground> {
    let path_buf = path.to_path_buf();
    let shared = Arc::new(SharedState {
        frame: Mutex::new(SharedFrame {
            seq: 0,
            size: [1, 1],
            rgb: Vec::new(),
        }),
        error: Mutex::new(None),
        stop: AtomicBool::new(false),
    });

    let thread_shared = Arc::clone(&shared);
    let thread_path = path_buf.clone();
    let handle = thread::Builder::new()
        .name("iris-bg-video".into())
        .spawn(move || {
            if let Err(e) = decode_loop(&thread_path, &thread_shared) {
                log::error!("Background video failed: {e:#}");
                *thread_shared.error.lock() = Some(format!("{e:#}"));
            }
        })
        .context("Starting background video thread")?;

    let start = Instant::now();
    loop {
        {
            let frame = shared.frame.lock();
            if frame.seq > 0 && !frame.rgb.is_empty() {
                let size = frame.size;
                let rgb = frame.rgb.clone();
                let seq = frame.seq;
                drop(frame);
                let color = ColorImage::from_rgb(size, &rgb);
                let texture = ctx.load_texture("background", color, TextureOptions::LINEAR);
                return Ok(VideoBackground {
                    path: path_buf,
                    texture,
                    size,
                    shared,
                    last_seq: seq,
                    thread: Some(handle),
                });
            }
        }
        if let Some(err) = shared.error.lock().clone() {
            shared.stop.store(true, Ordering::Relaxed);
            let _ = handle.join();
            bail!("{err}");
        }
        if handle.is_finished() {
            let err = shared
                .error
                .lock()
                .clone()
                .unwrap_or_else(|| "Video decoder stopped before the first frame".into());
            bail!("{err}");
        }
        if start.elapsed() > FIRST_FRAME_TIMEOUT {
            shared.stop.store(true, Ordering::Relaxed);
            let _ = handle.join();
            bail!("Timed out decoding the first video frame");
        }
        thread::sleep(Duration::from_millis(8));
        ctx.request_repaint();
    }
}

fn new_decoder() -> Result<Decoder> {
    Decoder::with_api_config(
        OpenH264API::from_source(),
        DecoderConfig::new().flush_after_decode(Flush::NoFlush),
    )
    .context("Creating H.264 decoder")
}

fn decode_opts() -> DecodeOptions {
    DecodeOptions::new().flush_after_decode(Flush::NoFlush)
}

fn decode_loop(path: &Path, shared: &SharedState) -> Result<()> {
    loop {
        if shared.stop.load(Ordering::Relaxed) {
            return Ok(());
        }
        match play_once(path, shared) {
            Ok(()) => {}
            Err(e) => {
                log::warn!("Background video pass failed: {e:#}");
                if shared.frame.lock().seq == 0 {
                    return Err(e);
                }
                sleep_interruptible(Duration::from_millis(250), &shared.stop);
            }
        }
    }
}

fn play_once(path: &Path, shared: &SharedState) -> Result<()> {
    let file = File::open(path).with_context(|| format!("Opening video {path:?}"))?;
    let file_size = file.metadata()?.len();
    let mut reader = mp4::Mp4Reader::read_header(file, file_size)
        .with_context(|| format!("Parsing MP4 {path:?}"))?;

    let track_id = reader
        .tracks()
        .iter()
        .find_map(|(id, track)| {
            if track.track_type().ok() == Some(mp4::TrackType::Video) {
                Some(*id)
            } else {
                None
            }
        })
        .ok_or_else(|| anyhow::anyhow!("No video track in {path:?}"))?;

    let (sample_count, converter, frame_dt) = {
        let track = reader
            .tracks()
            .get(&track_id)
            .ok_or_else(|| anyhow::anyhow!("Missing video track {track_id}"))?;
        match track.media_type() {
            Ok(mp4::MediaType::H264) => {}
            Ok(mp4::MediaType::H265) => {
                bail!("This MP4 is HEVC/H.265. Re-export as H.264 (AVC) to use it as a background.")
            }
            Ok(other) => bail!(
                "Unsupported MP4 codec ({other:?}). Use H.264 (AVC) for video backgrounds."
            ),
            Err(_) => {
                bail!("Could not read the MP4 video codec. Use a standard H.264 (AVC) MP4.")
            }
        }
        let sample_count = track.sample_count();
        if sample_count == 0 {
            bail!("Video track has no samples");
        }
        let fps = track.frame_rate();
        let frame_dt = Duration::from_secs_f64(if fps > 1.0 && fps < 240.0 {
            1.0 / fps
        } else {
            1.0 / 30.0
        });
        let converter = AnnexBConverter::from_track(track)?;
        if shared.frame.lock().seq == 0 {
            log::info!(
                "Background video {}x{} @ {:.2} fps, {} samples",
                track.width(),
                track.height(),
                fps,
                sample_count
            );
        }
        (sample_count, converter, frame_dt)
    };

    let mut decoder = new_decoder()?;
    let mut annexb = Vec::new();
    let mut rgb = Vec::new();
    let mut next_present = Instant::now();
    let mut force_headers = true;

    for sample_id in 1..=sample_count {
        if shared.stop.load(Ordering::Relaxed) {
            return Ok(());
        }
        let Some(sample) = reader
            .read_sample(track_id, sample_id)
            .with_context(|| format!("Reading sample {sample_id}"))?
        else {
            continue;
        };

        converter.convert_packet(&sample.bytes, &mut annexb, force_headers);
        force_headers = false;
        if annexb.is_empty() {
            continue;
        }

        match decoder.decode_with_options(&annexb, decode_opts()) {
            Ok(Some(yuv)) => {
                publish_frame(shared, &yuv, &mut rgb, &mut next_present, frame_dt);
            }
            Ok(None) => {}
            Err(e) => {
                log::debug!("H.264 decode skip at sample {sample_id}: {e}");
            }
        }
    }

    if let Ok(frames) = decoder.flush_remaining() {
        for yuv in &frames {
            if shared.stop.load(Ordering::Relaxed) {
                return Ok(());
            }
            publish_frame(shared, yuv, &mut rgb, &mut next_present, frame_dt);
        }
    }

    Ok(())
}

fn publish_frame(
    shared: &SharedState,
    yuv: &openh264::decoder::DecodedYUV<'_>,
    rgb: &mut Vec<u8>,
    next_present: &mut Instant,
    frame_dt: Duration,
) {
    let (w, h) = yuv.dimensions();
    if w == 0 || h == 0 {
        return;
    }
    rgb.resize(yuv.rgb8_len(), 0);
    yuv.write_rgb8(rgb);
    let (rgb_out, nw, nh) = maybe_downscale(std::mem::take(rgb), w as u32, h as u32);
    if rgb_out.is_empty() {
        return;
    }

    let now = Instant::now();
    if *next_present > now {
        sleep_interruptible(next_present.saturating_duration_since(now), &shared.stop);
    }
    *next_present += frame_dt;
    if Instant::now().saturating_duration_since(*next_present) > Duration::from_millis(250) {
        *next_present = Instant::now() + frame_dt;
    }

    if shared.stop.load(Ordering::Relaxed) {
        return;
    }
    let mut g = shared.frame.lock();
    g.rgb = rgb_out;
    g.size = [nw as usize, nh as usize];
    g.seq = g.seq.saturating_add(1);
}

fn sleep_interruptible(total: Duration, stop: &AtomicBool) {
    let deadline = Instant::now() + total;
    while Instant::now() < deadline {
        if stop.load(Ordering::Relaxed) {
            return;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        thread::sleep(remaining.min(Duration::from_millis(8)));
    }
}

fn maybe_downscale(rgb: Vec<u8>, w: u32, h: u32) -> (Vec<u8>, u32, u32) {
    let max_side = VIDEO_MAX_SIDE.min(SAFE_MAX_SIDE);
    if w == 0 || h == 0 {
        return (rgb, w.max(1), h.max(1));
    }
    if w <= max_side && h <= max_side {
        return (rgb, w, h);
    }
    let Some(img) = RgbImage::from_raw(w, h, rgb) else {
        return (Vec::new(), w, h);
    };
    let scale = (max_side as f32 / w as f32).min(max_side as f32 / h as f32);
    let nw = ((w as f32 * scale).floor() as u32).max(1);
    let nh = ((h as f32 * scale).floor() as u32).max(1);
    let scaled = DynamicImage::ImageRgb8(img).resize(nw, nh, FilterType::Triangle);
    let rgb8 = scaled.to_rgb8();
    let (ow, oh) = rgb8.dimensions();
    (rgb8.into_raw(), ow, oh)
}

/// MP4 length-prefixed NALs → Annex-B (`00 00 00 01`) for OpenH264.
struct AnnexBConverter {
    length_size: usize,
    sps: Vec<Vec<u8>>,
    pps: Vec<Vec<u8>>,
}

impl AnnexBConverter {
    fn from_track(track: &mp4::Mp4Track) -> Result<Self> {
        let avc1 = track
            .trak
            .mdia
            .minf
            .stbl
            .stsd
            .avc1
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Video track is not AVC1/H.264"))?;
        let length_size = (avc1.avcc.length_size_minus_one as usize & 3) + 1;
        let sps = avc1
            .avcc
            .sequence_parameter_sets
            .iter()
            .map(|n| n.bytes.clone())
            .collect();
        let pps = avc1
            .avcc
            .picture_parameter_sets
            .iter()
            .map(|n| n.bytes.clone())
            .collect();
        Ok(Self {
            length_size,
            sps,
            pps,
        })
    }

    fn convert_packet(&self, packet: &[u8], out: &mut Vec<u8>, force_headers: bool) {
        out.clear();
        let mut i = 0;
        let mut nals: Vec<&[u8]> = Vec::new();
        let mut saw_sps = false;
        let mut saw_pps = false;
        let mut has_idr = false;
        while i + self.length_size <= packet.len() {
            let mut len = 0usize;
            for j in 0..self.length_size {
                len = (len << 8) | packet[i + j] as usize;
            }
            i += self.length_size;
            if len == 0 || i + len > packet.len() {
                break;
            }
            let nal = &packet[i..i + len];
            i += len;
            if nal.is_empty() {
                continue;
            }
            let nal_type = nal[0] & 0x1F;
            match nal_type {
                5 => has_idr = true,
                7 => saw_sps = true,
                8 => saw_pps = true,
                _ => {}
            }
            nals.push(nal);
        }

        if force_headers || (has_idr && (!saw_sps || !saw_pps)) {
            for sps in &self.sps {
                write_annexb(out, sps);
            }
            for pps in &self.pps {
                write_annexb(out, pps);
            }
        }
        for nal in nals {
            write_annexb(out, nal);
        }
    }
}

fn write_annexb(out: &mut Vec<u8>, nal: &[u8]) {
    out.extend_from_slice(&[0, 0, 0, 1]);
    out.extend_from_slice(nal);
}
