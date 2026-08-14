//! Iris Visualizer — real-time DJ music visualizer.

mod app;
mod audio;
mod config;
mod display;
mod renderer;

use anyhow::Result;
use app::LiveVisualizerApp;
use eframe::NativeOptions;
use egui::ViewportBuilder;

fn main() -> Result<()> {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info,wgpu_core=warn,wgpu_hal=warn,naga=warn"),
    )
    .init();

    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")).ok();
    let mut viewport = ViewportBuilder::default()
        .with_inner_size([1280.0, 720.0])
        .with_min_inner_size([800.0, 500.0])
        .with_title("Iris Visualizer");
    if let Some(icon) = icon {
        viewport = viewport.with_icon(icon);
    }

    let options = NativeOptions {
        viewport,
        vsync: true,
        // Prefer wgpu; eframe will fall back if needed.
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };

    eframe::run_native(
        "Iris Visualizer",
        options,
        Box::new(|cc| Ok(Box::new(LiveVisualizerApp::new(cc)))),
    )
    .map_err(|e| anyhow::anyhow!("Failed to start Iris Visualizer: {e}"))?;

    Ok(())
}
