//! Placeholder for the future wgpu visualizer renderer.

/// Owns GPU resources for background, logo, spectrum, particles, bloom.
/// Not wired in Phase 1–2 (egui preview is used instead).
#[derive(Default)]
pub struct VisualizerRenderer {
    pub ready: bool,
}

impl VisualizerRenderer {
    pub fn new() -> Self {
        Self { ready: false }
    }
}
