//! Loaded stage images (background + logo) as egui textures.
//!
//! Supports common rasters (PNG/JPEG/WebP/BMP) and SVG (via `resvg`).
//! Large images are downscaled to fit the GPU / egui max texture side
//! (commonly 2048–8192) so a 4K logo cannot panic the app at launch.


use anyhow::{bail, Context, Result};
use egui::{ColorImage, TextureHandle, TextureOptions};
use image::imageops::FilterType;
use image::{DynamicImage, RgbaImage};
use std::path::{Path, PathBuf};

/// Stay under common Metal/egui limits with a little headroom.
const SAFE_MAX_SIDE: u32 = 2048;

#[derive(Clone)]
pub struct LoadedTexture {
    #[allow(dead_code)]
    pub path: PathBuf,
    pub texture: TextureHandle,
    pub size: [usize; 2],
}

pub fn load_texture_from_path(
    ctx: &egui::Context,
    path: &Path,
    name: &str,
) -> Result<LoadedTexture> {
    let bytes = std::fs::read(path).with_context(|| format!("Reading image {path:?}"))?;
    load_texture_from_bytes(ctx, &bytes, path.to_path_buf(), name)
}

pub fn load_texture_from_bytes(
    ctx: &egui::Context,
    bytes: &[u8],
    path: PathBuf,
    name: &str,
) -> Result<LoadedTexture> {
    let max_side = ctx
        .input(|i| i.max_texture_side)
        .min(SAFE_MAX_SIDE as usize)
        .max(64) as u32;

    let img = if looks_like_svg(&path, bytes) {
        decode_svg(bytes, max_side)
            .with_context(|| format!("Decoding SVG {path:?}"))?
    } else {
        let img = image::load_from_memory(bytes)
            .with_context(|| format!("Decoding image {path:?}"))?;
        downscale_dynamic(img, max_side, name)
    };

    let rgba = img.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let color = ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    let texture = ctx.load_texture(name, color, TextureOptions::LINEAR);
    Ok(LoadedTexture {
        path,
        texture,
        size,
    })
}

fn looks_like_svg(path: &Path, bytes: &[u8]) -> bool {
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        if ext.eq_ignore_ascii_case("svg") || ext.eq_ignore_ascii_case("svgz") {
            return true;
        }
    }
    let head = &bytes[..bytes.len().min(1024)];
    let lower: Vec<u8> = head.iter().map(|b| b.to_ascii_lowercase()).collect();
    lower.windows(4).any(|w| w == b"<svg")
}

fn downscale_dynamic(img: DynamicImage, max_side: u32, name: &str) -> DynamicImage {
    let (ow, oh) = (img.width(), img.height());
    if ow <= max_side && oh <= max_side {
        return img;
    }
    let scale = (max_side as f32 / ow as f32).min(max_side as f32 / oh as f32);
    let nw = ((ow as f32 * scale).floor() as u32).max(1);
    let nh = ((oh as f32 * scale).floor() as u32).max(1);
    log::info!("Downscaling {name} from {ow}x{oh} → {nw}x{nh} (max texture side {max_side})");
    img.resize(nw, nh, FilterType::Triangle)
}

/// Rasterize SVG to RGBA. Animation timelines are ignored (static snapshot).
fn decode_svg(bytes: &[u8], max_side: u32) -> Result<DynamicImage> {
  // don't crash the app
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        decode_svg_inner(bytes, max_side)
    }));
    match result {
        Ok(inner) => inner,
        Err(_) => bail!("SVG renderer panicked (file may be malformed or too complex)"),
    }
}

fn decode_svg_inner(bytes: &[u8], max_side: u32) -> Result<DynamicImage> {
    let opt = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_data(bytes, &opt)
        .map_err(|e| anyhow::anyhow!("SVG parse failed: {e}"))?;

    let size = tree.size();
    let ow = size.width().max(1.0);
    let oh = size.height().max(1.0);

    let scale = (max_side as f32 / ow).min(max_side as f32 / oh).min(1.0);
    // Prefer at least ~512px on the long side for sharp logos when the SVG is tiny.
    let scale = if ow.max(oh) * scale < 512.0 && ow.max(oh) < max_side as f32 {
        (512.0 / ow.max(oh)).min(max_side as f32 / ow.max(oh))
    } else {
        scale
    };

    let nw = ((ow * scale).ceil() as u32).clamp(1, max_side);
    let nh = ((oh * scale).ceil() as u32).clamp(1, max_side);
    let sx = nw as f32 / ow;
    let sy = nh as f32 / oh;

    let mut pixmap = resvg::tiny_skia::Pixmap::new(nw, nh)
        .ok_or_else(|| anyhow::anyhow!("Failed to allocate {nw}x{nh} SVG pixmap"))?;

    let transform = resvg::tiny_skia::Transform::from_scale(sx, sy);
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    let rgba = RgbaImage::from_raw(nw, nh, pixmap.take())
        .ok_or_else(|| anyhow::anyhow!("SVG pixmap size mismatch"))?;
    Ok(DynamicImage::ImageRgba8(rgba))
}

/// Aspect-fit rect inside `bounds`.
pub fn fit_rect(bounds: egui::Rect, image_size: [usize; 2]) -> egui::Rect {
    let iw = image_size[0].max(1) as f32;
    let ih = image_size[1].max(1) as f32;
    let scale = (bounds.width() / iw).min(bounds.height() / ih);
    let w = iw * scale;
    let h = ih * scale;
    egui::Rect::from_center_size(bounds.center(), egui::vec2(w, h))
}

/// Cover rect (fill bounds, crop overflow) with optional zoom + pan.
pub fn cover_rect(
    bounds: egui::Rect,
    image_size: [usize; 2],
    zoom: f32,
    pan_x: f32,
    pan_y: f32,
) -> egui::Rect {
    let iw = image_size[0].max(1) as f32;
    let ih = image_size[1].max(1) as f32;
    let scale = (bounds.width() / iw).max(bounds.height() / ih) * zoom.max(0.5);
    let w = iw * scale;
    let h = ih * scale;
    let mut rect = egui::Rect::from_center_size(bounds.center(), egui::vec2(w, h));
    rect = rect.translate(egui::vec2(
        pan_x * bounds.width() * 0.25,
        pan_y * bounds.height() * 0.25,
    ));
    rect
}
