//! Batched radial spectrum bars drawn through egui.
//!
//! Consumes the shared `AudioFeatures.spectrum` — it never runs its own FFT.

use crate::audio::{band_center_hz, frequency_height_scale, AudioFeatures};
use crate::config::{
    neon_ring_gradient, BarCap, ColorMode, ColorSettings, RgbColor, SpectrumLayout, SpectrumStyle,
    VisualizerSettings,
};
use egui::epaint::{Color32, Mesh, Shape, Stroke, Vertex, WHITE_UV};
use egui::{Pos2, Vec2};

/// Reusable mesh buffers for the circular bar ring.
pub struct RadialSpectrum {
    glow: Mesh,
    core: Mesh,
    caps: Mesh,
    amps: Vec<f32>,
    ears: Vec<f32>,
    heights: Vec<f32>,
    freq_t: Vec<f32>,
}

impl Default for RadialSpectrum {
    fn default() -> Self {
        Self::new()
    }
}

impl RadialSpectrum {
    pub fn new() -> Self {
        Self {
            glow: Mesh::default(),
            core: Mesh::default(),
            caps: Mesh::default(),
            amps: Vec::with_capacity(256),
            ears: Vec::with_capacity(256),
            heights: Vec::with_capacity(256),
            freq_t: Vec::with_capacity(256),
        }
    }

    pub fn paint(
        &mut self,
        painter: &egui::Painter,
        center: Pos2,
        disc_r: f32,
        min_dim: f32,
        features: &AudioFeatures,
        vis: &VisualizerSettings,
        colors: &ColorSettings,
        time: f32,
    ) {
        let spectrum = &features.spectrum;
        if spectrum.is_empty() {
            return;
        }

        let n = vis.segment_count.clamp(48, 512) as usize;
        let step = std::f32::consts::TAU / n as f32;
        let fill = vis.bar_fill.clamp(0.45, 0.92);
        let half_ang = step * fill * 0.5;
        let rotation = vis.rotation_offset.fract() * std::f32::consts::TAU;
        let base_r = disc_r + min_dim * 0.006;
        let min_h = (min_dim * vis.min_bar_height.clamp(0.003, 0.04)).max(3.0);
        let mut max_h = (disc_r * vis.max_bar_length.clamp(0.12, 1.0))
            .min(min_dim * 0.18)
            .max(min_h * 4.0);
        if vis.layout == SpectrumLayout::Split {
            max_h *= 1.22;
        }
        let glow_amt = (vis.glow * colors.glow_intensity).clamp(0.0, 2.0);
        let cycle = time * vis.cycle_speed.clamp(0.0, 3.0);
        let ear_lift = disc_r * vis.ear_radius.clamp(0.0, 0.35);
        let peaks = &features.spectrum_peaks;

        self.amps.resize(n, 0.0);
        self.ears.resize(n, 0.0);
        self.heights.resize(n, 0.0);
        self.freq_t.resize(n, 0.0);

        for i in 0..n {
            let (amp, ear, freq_t) = layout_sample(i, n, spectrum, vis, features);
            let freq = band_center_hz(
                (freq_t * spectrum.len().saturating_sub(1) as f32) as usize,
                spectrum.len().max(1),
                vis.min_frequency,
                vis.max_frequency,
            );
            let scale = if vis.layout == SpectrumLayout::Split {
                1.0
            } else {
                frequency_height_scale(freq)
            };
            self.amps[i] = amp;
            self.ears[i] = ear;
            self.freq_t[i] = freq_t;
            self.heights[i] = min_h + amp.clamp(0.0, 1.0) * max_h * scale;
        }

        self.glow.clear();
        self.core.clear();
        self.caps.clear();

        if vis.show_base_ring {
            let br = vis.base_ring_brightness.clamp(0.02, 0.6);
            let (r, g, b) = neon_ring_gradient(0.12);
            painter.circle_stroke(
                center,
                base_r,
                Stroke::new(
                    vis.base_ring_width.clamp(0.4, 3.0),
                    Color32::from_rgba_unmultiplied(
                        (r * 255.0) as u8,
                        (g * 255.0) as u8,
                        (b * 255.0) as u8,
                        (br * 90.0) as u8,
                    ),
                ),
            );
        }

        for i in 0..n {
            let amp = self.amps[i];
            let ear = self.ears[i];
            let height = self.heights[i];
            let freq_t = self.freq_t[i];
            let angle = -std::f32::consts::FRAC_PI_2 + rotation + (i as f32 + 0.5) * step;
            let inner_r = if vis.layout == SpectrumLayout::Split {
                base_r + ear_lift * ear
            } else {
                base_r
            };
            let outer_r = inner_r + height;
            let rgb = ring_color(vis.color_mode, colors, freq_t, cycle, amp);
            let emit = (0.74 + amp * 0.36 + features.transient_energy * 0.08).clamp(0.55, 1.25);
            let core = boost_rgb(rgb, emit, amp);

            if glow_amt > 0.04 {
                let glow_extra = (min_dim * 0.0035 + height * 0.05 * glow_amt).min(height * 0.22);
                let halo = Color32::from_rgba_unmultiplied(
                    core[0],
                    core[1],
                    core[2],
                    (26.0 * glow_amt).clamp(8.0, 48.0) as u8,
                );
                push_bar(
                    &mut self.glow,
                    center,
                    angle,
                    half_ang * (1.0 + 0.22 * glow_amt.min(1.0)),
                    (inner_r - 1.0).max(1.0),
                    outer_r + glow_extra,
                    halo,
                );
                let inner_glow = Color32::from_rgba_unmultiplied(
                    core[0],
                    core[1],
                    core[2],
                    (62.0 * glow_amt.min(1.2)).clamp(16.0, 96.0) as u8,
                );
                push_bar(
                    &mut self.glow,
                    center,
                    angle,
                    half_ang * 1.06,
                    inner_r,
                    outer_r + glow_extra * 0.28,
                    inner_glow,
                );
            }

            let core_c = Color32::from_rgba_unmultiplied(core[0], core[1], core[2], 255);
            push_bar(
                &mut self.core,
                center,
                angle,
                half_ang,
                inner_r,
                outer_r,
                core_c,
            );

            if vis.bar_cap == BarCap::Rounded {
                let (s, c) = angle.sin_cos();
                let cap_r = (base_r * half_ang).abs().clamp(1.1, 3.2);
                add_circle(
                    &mut self.caps,
                    center + Vec2::new(c, s) * outer_r,
                    cap_r,
                    core_c,
                    7,
                );
            }

            if vis.show_peak_caps && !peaks.is_empty() {
                let peak = sample_band(peaks, (i as f32 + 0.5) / n as f32);
                let peak_h = min_h + peak * max_h * 0.92;
                if peak_h > height + 2.5 {
                    let (s, c) = angle.sin_cos();
                    add_circle(
                        &mut self.caps,
                        center + Vec2::new(c, s) * (inner_r + peak_h),
                        1.7,
                        Color32::from_rgba_unmultiplied(core[0], core[1], core[2], 210),
                        6,
                    );
                }
            }
        }

        if !self.glow.vertices.is_empty() {
            painter.add(Shape::mesh(self.glow.clone()));
        }
        if !self.core.vertices.is_empty() {
            painter.add(Shape::mesh(self.core.clone()));
        }
        if !self.caps.vertices.is_empty() {
            painter.add(Shape::mesh(self.caps.clone()));
        }
    }
}

fn layout_sample(
    i: usize,
    n: usize,
    spectrum: &[f32],
    vis: &VisualizerSettings,
    features: &AudioFeatures,
) -> (f32, f32, f32) {
    match vis.layout {
        SpectrumLayout::Continuous => {
            let t = i as f32 / n.max(1) as f32;
            let t = if vis.reverse_spectrum { 1.0 - t } else { t };
            let t = remap_ring_t(t, vis);
            (sample_band(spectrum, t), 0.0, t)
        }
        SpectrumLayout::Mirrored => {
            let half = (n / 2).max(1);
            let dist = if i <= half { i } else { n - i };
            let mut t = dist as f32 / half as f32;
            if vis.reverse_spectrum {
                t = 1.0 - t;
            }
            let t = remap_ring_t(t, vis);
            (sample_band(spectrum, t), 0.0, t)
        }
        SpectrumLayout::Split => {
            let t = (i as f32 + 0.5) / n as f32;
            let u = if t <= 0.5 { t * 2.0 } else { (1.0 - t) * 2.0 };
            let u = if vis.reverse_spectrum { 1.0 - u } else { u };
            let cut = ((spectrum.len() as f32) * 0.20) as usize;
            let rest = (spectrum.len() - cut).max(8);
            let punch = (features.bass * 0.5 + features.kick * 0.95).min(1.2);
            let (mag, ear) = trap_nation_mag(
                spectrum,
                cut,
                rest,
                u,
                punch,
                vis.ear_gain.clamp(0.2, 2.8),
                if vis.ear_count >= 4 { 4 } else { 2 },
                vis.ear_angle.clamp(0.08, 0.42),
                vis.ear_width.clamp(0.035, 0.16),
            );
            (mag.clamp(0.0, 1.0), ear, (0.06 + u * 0.82).clamp(0.0, 1.0))
        }
    }
}

/// In bar mode, skip sub rumble (20–~65 Hz). Those bins jump as a block
/// rather than showing frequency detail, so the ring starts at kick/bass.
fn remap_ring_t(t: f32, vis: &VisualizerSettings) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if vis.style != SpectrumStyle::Bars {
        return t;
    }
    let skip = log_t_for_hz(65.0, vis.min_frequency, vis.max_frequency);
    skip + t * (1.0 - skip)
}

fn log_t_for_hz(hz: f32, min_hz: f32, max_hz: f32) -> f32 {
    let min_hz = min_hz.max(20.0);
    let max_hz = max_hz.max(min_hz * 2.0);
    if hz <= min_hz {
        return 0.0;
    }
    let hz = hz.min(max_hz);
    ((hz / min_hz).ln() / (max_hz / min_hz).ln()).clamp(0.0, 0.45)
}

fn sample_band(bands: &[f32], t: f32) -> f32 {
    interp_spec(bands, t.clamp(0.0, 1.0) * (bands.len().saturating_sub(1) as f32))
}

fn interp_spec(s: &[f32], idx: f32) -> f32 {
    if s.is_empty() {
        return 0.0;
    }
    let max_i = (s.len() - 1) as f32;
    let idx = idx.clamp(0.0, max_i);
    let i = idx.floor() as usize;
    let f = idx - i as f32;
    let a = s[i];
    let b = s.get(i + 1).copied().unwrap_or(a);
    (a * (1.0 - f) + b * f).clamp(0.0, 1.0)
}

fn trap_nation_mag(
    smoothed: &[f32],
    cut: usize,
    rest: usize,
    u: f32,
    punch: f32,
    ear_gain: f32,
    ear_count: u8,
    ear_angle: f32,
    ear_width: f32,
) -> (f32, f32) {
    let ear = if ear_count >= 4 {
        let c1 = (ear_angle * 0.72).clamp(0.08, 0.32);
        let c2 = (ear_angle * 1.38).clamp(0.18, 0.48);
        ear_gauss(u, c1, ear_width) * 0.92 + ear_gauss(u, c2, ear_width * 0.95) * 0.78
    } else {
        ear_gauss(u, ear_angle, ear_width)
    };
    let ear = ear.clamp(0.0, 1.35);
    let span = ear_width * 4.4;
    let bass_u = ((u - (ear_angle - ear_width * 2.2)) / span).clamp(0.0, 1.0);
    let bass_tex = interp_spec(smoothed, bass_u * (cut.saturating_sub(1) as f32));
    let ear_mag = (bass_tex * 0.58 + punch * 0.7) * ear * ear_gain;
    let body = interp_spec(smoothed, cut as f32 + u * (rest.saturating_sub(1) as f32));
    let body = body * (1.0 - ear * 0.58);
    ((ear_mag + body).clamp(0.0, 1.45), ear.min(1.0))
}

fn ear_gauss(u: f32, center: f32, width: f32) -> f32 {
    let x = (u - center) / width.max(0.02);
    (-x * x * 2.1).exp()
}

fn push_bar(
    mesh: &mut Mesh,
    center: Pos2,
    angle: f32,
    half_ang: f32,
    inner_r: f32,
    outer_r: f32,
    color: Color32,
) {
    let a0 = angle - half_ang;
    let a1 = angle + half_ang;
    let (s0, c0) = a0.sin_cos();
    let (s1, c1) = a1.sin_cos();
    let inner_left = Pos2::new(center.x + c0 * inner_r, center.y + s0 * inner_r);
    let inner_right = Pos2::new(center.x + c1 * inner_r, center.y + s1 * inner_r);
    let outer_left = Pos2::new(center.x + c0 * outer_r, center.y + s0 * outer_r);
    let outer_right = Pos2::new(center.x + c1 * outer_r, center.y + s1 * outer_r);
    let i = mesh.vertices.len() as u32;
    mesh.vertices.extend_from_slice(&[
        vert(inner_left, color),
        vert(inner_right, color),
        vert(outer_right, color),
        vert(outer_left, color),
    ]);
    mesh.indices
        .extend_from_slice(&[i, i + 1, i + 2, i, i + 2, i + 3]);
}

fn add_circle(mesh: &mut Mesh, center: Pos2, radius: f32, color: Color32, segments: usize) {
    let segments = segments.max(5);
    let start = mesh.vertices.len() as u32;
    mesh.vertices.push(vert(center, color));
    for i in 0..segments {
        let a = i as f32 / segments as f32 * std::f32::consts::TAU;
        let (s, c) = a.sin_cos();
        mesh.vertices
            .push(vert(Pos2::new(center.x + c * radius, center.y + s * radius), color));
    }
    for i in 0..segments {
        let a = start + 1 + i as u32;
        let b = start + 1 + ((i + 1) % segments) as u32;
        mesh.indices.extend_from_slice(&[start, a, b]);
    }
}

fn vert(pos: Pos2, color: Color32) -> Vertex {
    Vertex {
        pos,
        uv: WHITE_UV,
        color,
    }
}

fn ring_color(
    mode: ColorMode,
    colors: &ColorSettings,
    freq_t: f32,
    cycle: f32,
    amp: f32,
) -> RgbColor {
    let intensity = (0.82 + amp * 0.22).clamp(0.5, 1.2);
    let t = freq_t.clamp(0.0, 1.0);
    match mode {
        ColorMode::Rainbow => {
            let (r, g, b) = neon_ring_gradient(t);
            RgbColor::new(r, g, b)
        }
        ColorMode::Cycle => {
            let (r, g, b) = neon_ring_gradient((t + cycle).rem_euclid(1.0));
            RgbColor::new(r, g, b)
        }
        ColorMode::Cyan => RgbColor::new(0.2, 0.92, 1.0),
        ColorMode::Amber => RgbColor::new(1.0, 0.67, 0.22),
        ColorMode::Magenta => RgbColor::new(1.0, 0.22, 0.75),
        _ => colors.sample_cycled(mode, t, intensity, cycle),
    }
}

fn boost_rgb(c: RgbColor, emit: f32, amp: f32) -> [u8; 3] {
    let lift = amp * 0.16;
    let r = (c.r * emit + lift).clamp(0.0, 1.0);
    let g = (c.g * emit + lift).clamp(0.0, 1.0);
    let b = (c.b * emit + lift).clamp(0.0, 1.0);
    [(r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8]
}
