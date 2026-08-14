//! Loaded stage images (background + logo) as egui textures.
//!
//! Large images are downscaled to fit the GPU / egui max texture side
//! (commonly 2048–8192) so a 4K logo cannot panic the app at launch.

use anyhow::{Context, Result};
use egui::{ColorImage, TextureHandle, TextureOptions};
use image::imageops::FilterType;
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

    let img = image::load_from_memory(bytes)
        .with_context(|| format!("Decoding image {path:?}"))?;

    let (ow, oh) = (img.width(), img.height());
    let img = if ow > max_side || oh > max_side {
        let scale = (max_side as f32 / ow as f32).min(max_side as f32 / oh as f32);
        let nw = ((ow as f32 * scale).floor() as u32).max(1);
        let nh = ((oh as f32 * scale).floor() as u32).max(1);
        log::info!(
            "Downscaling {name} from {ow}x{oh} → {nw}x{nh} (max texture side {max_side})"
        );
        img.resize(nw, nh, FilterType::Triangle)
    } else {
        img
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
