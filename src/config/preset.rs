//! Application settings and preset file format.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::audio::BandTuning;
use crate::audio::BeatMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ColorMode {
    /// Neon rainbow around the ring / along the FFT.
    #[default]
    #[serde(alias = "rgb")]
    Rainbow,
    /// Single custom color (see `ColorSettings::solid`).
    Solid,
    /// Blend from low → high (see gradient colors).
    Gradient,
    /// Rainbow that scrolls over time.
    Cycle,
    /// Five frequency-band colors blended by position.
    Bands,
    Mono,
    /// Legacy presets — still render; UI maps to Solid.
    Cyan,
    Amber,
    Magenta,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SpectrumStyle {
    #[default]
    Bars,
    Smooth,
    SoftGlow,
}

/// How the spectrum is laid out around the disc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SpectrumLayout {
    /// Frequency increases clockwise around the full 360° ring.
    #[serde(alias = "full")]
    Continuous,
    /// Half the spectrum mirrored across the center. Bass sits at the seam.
    #[default]
    Mirrored,
    /// Legacy Trap Nation bass-ear layout.
    Split,
}

/// Outer cap of each radial bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BarCap {
    #[default]
    Square,
    Rounded,
}

/// Color style for the center oscilloscope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ScopeColorMode {
    #[default]
    Solid,
    /// Neon rainbow along the waveform.
    Rainbow,
    /// Blend `scope_color` → `scope_color_b`.
    Gradient,
    /// Rainbow that scrolls over time.
    Cycle,
}

/// Playlist sentinel for a center oscilloscope slot (not a file path).
pub const CENTER_SCOPE_ID: &str = ":scope:";
/// Playlist sentinel for a mirrored condensed FFT slot.
pub const CENTER_SPECTRUM_ID: &str = ":spectrum:";

pub fn is_center_scope(path: &str) -> bool {
    path == CENTER_SCOPE_ID
}

pub fn is_center_spectrum(path: &str) -> bool {
    path == CENTER_SPECTRUM_ID
}

pub fn is_center_effect(path: &str) -> bool {
    is_center_scope(path) || is_center_spectrum(path)
}

pub fn center_item_label(path: &str) -> String {
    if is_center_scope(path) {
        "Oscilloscope".into()
    } else if is_center_spectrum(path) {
        "Spectrum".into()
    } else {
        path.to_string()
    }
}

/// Motion applied to the center logo / disc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LogoMotion {
    #[default]
    None,
    /// Constant spin.
    Spin,
    /// Spin rate boosted by beat energy.
    BeatSpin,
    /// Gentle left/right sway.
    Wobble,
    /// Slow pendulum swing.
    Pendulum,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub last_audio_device: Option<String>,
    pub last_preset: Option<String>,
    pub show_debug: bool,
    #[serde(default)]
    pub tuning: BandTuning,
    #[serde(default)]
    pub background_path: Option<String>,
    /// Legacy single logo — migrated into `logo_paths` on load.
    #[serde(default)]
    pub logo_path: Option<String>,
    /// Playlist of center logos (cycles with optional glitch).
    #[serde(default)]
    pub logo_paths: Vec<String>,
    /// Per-logo styles (parallel to `logo_paths`).
    #[serde(default)]
    pub logo_styles: Vec<LogoStyle>,
    #[serde(default)]
    pub stage: StageSettings,
    #[serde(default)]
    pub visualizer: VisualizerSettings,
    #[serde(default)]
    pub particles: ParticleSettings,
    #[serde(default)]
    pub beat: BeatSettings,
    #[serde(default)]
    pub colors: ColorSettings,
}

/// Per-logo look — each playlist entry can be tuned on its own.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogoStyle {
    #[serde(default = "default_logo_size")]
    pub size: f32,
    #[serde(default)]
    pub offset_x: f32,
    #[serde(default)]
    pub offset_y: f32,
    #[serde(default = "default_one")]
    pub tint_r: f32,
    #[serde(default = "default_one")]
    pub tint_g: f32,
    #[serde(default = "default_one")]
    pub tint_b: f32,
    #[serde(default = "default_one")]
    pub opacity: f32,
    #[serde(default = "default_one")]
    pub brightness: f32,
    #[serde(default)]
    pub motion: LogoMotion,
    #[serde(default = "default_spin_speed")]
    pub spin_speed: f32,
    #[serde(default = "default_motion_amount")]
    pub motion_amount: f32,
    /// Override hold time for this logo (0 = use playlist default).
    #[serde(default)]
    pub hold_secs: f32,
    /// Fire a glitch burst when a kick/beat hits while this logo is up.
    #[serde(default = "default_true_stage")]
    pub glitch_on_beat: bool,
}

fn default_logo_size() -> f32 {
    0.72
}

impl Default for LogoStyle {
    fn default() -> Self {
        Self {
            size: 0.72,
            offset_x: 0.0,
            offset_y: 0.0,
            tint_r: 1.0,
            tint_g: 1.0,
            tint_b: 1.0,
            opacity: 1.0,
            brightness: 1.0,
            motion: LogoMotion::None,
            spin_speed: 0.15,
            motion_amount: 0.35,
            hold_secs: 0.0,
            glitch_on_beat: true,
        }
    }
}

impl LogoStyle {
    pub fn from_stage(s: &StageSettings) -> Self {
        Self {
            size: s.logo_size,
            offset_x: s.logo_offset_x,
            offset_y: s.logo_offset_y,
            tint_r: s.logo_tint_r,
            tint_g: s.logo_tint_g,
            tint_b: s.logo_tint_b,
            opacity: s.logo_opacity,
            brightness: s.logo_brightness,
            motion: s.logo_motion,
            spin_speed: s.logo_spin_speed,
            motion_amount: s.logo_motion_amount,
            hold_secs: 0.0,
            glitch_on_beat: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageSettings {
    pub background_brightness: f32,
    pub background_darkness: f32,
    pub background_zoom: f32,
    pub background_pan_x: f32,
    pub background_pan_y: f32,
    pub logo_size: f32,
    pub disc_radius: f32,
    pub logo_glow: f32,
    pub bass_pulse: f32,
    pub beat_pulse: f32,
    /// Logo shift as fraction of disc radius (−1…1).
    #[serde(default)]
    pub logo_offset_x: f32,
    #[serde(default)]
    pub logo_offset_y: f32,
    /// Multiplicative tint (1,1,1 = original / white).
    #[serde(default = "default_one")]
    pub logo_tint_r: f32,
    #[serde(default = "default_one")]
    pub logo_tint_g: f32,
    #[serde(default = "default_one")]
    pub logo_tint_b: f32,
    #[serde(default = "default_one")]
    pub logo_opacity: f32,
    #[serde(default = "default_one")]
    pub logo_brightness: f32,
    /// Center logo / disc motion.
    #[serde(default)]
    pub logo_motion: LogoMotion,
    /// Revolutions per second when spinning (also scales wobble/pendulum).
    #[serde(default = "default_spin_speed")]
    pub logo_spin_speed: f32,
    /// How strong wobble / pendulum swing is (0..1).
    #[serde(default = "default_motion_amount")]
    pub logo_motion_amount: f32,
    /// Optional thin tick marks on the black disc that spin with the logo.
    #[serde(default)]
    pub disc_ticks: bool,
    /// Center disc fill color (under the logo).
    #[serde(default = "default_disc_color")]
    pub disc_color: RgbColor,
    #[serde(default = "default_scope_gain")]
    pub scope_gain: f32,
    #[serde(default = "default_scope_thickness")]
    pub scope_thickness: f32,
    #[serde(default)]
    pub scope_color_mode: ScopeColorMode,
    #[serde(default = "default_scope_color")]
    pub scope_color: RgbColor,
    #[serde(default = "default_scope_color_b")]
    pub scope_color_b: RgbColor,
    #[serde(default = "default_scope_cycle_speed")]
    pub scope_cycle_speed: f32,
    /// Separate look for the center spectrum bars.
    #[serde(default = "default_scope_gain")]
    pub spectrum_gain: f32,
    #[serde(default = "default_scope_thickness")]
    pub spectrum_thickness: f32,
    #[serde(default)]
    pub spectrum_color_mode: ScopeColorMode,
    #[serde(default = "default_spectrum_color")]
    pub spectrum_color: RgbColor,
    #[serde(default = "default_spectrum_color_b")]
    pub spectrum_color_b: RgbColor,
    #[serde(default = "default_scope_cycle_speed")]
    pub spectrum_cycle_speed: f32,
    /// Seconds each logo stays before cycling (multi-logo).
    #[serde(default = "default_logo_hold")]
    pub logo_hold_secs: f32,
    /// When true, playlist advances on the hold timer.
    #[serde(default = "default_true_stage")]
    pub logo_autoplay: bool,
    /// CRT-style flatten → line → center when switching to/from effects.
    #[serde(default = "default_true_stage")]
    pub crt_transition: bool,
    /// Glitch burst when switching logos (transition).
    #[serde(default = "default_true_stage")]
    pub logo_glitch: bool,
    #[serde(default = "default_one")]
    pub logo_glitch_amount: f32,
    /// Separate beat-hit glitch (does not share the transition envelope).
    #[serde(default = "default_true_stage")]
    pub logo_glitch_on_beat: bool,
    #[serde(default = "default_one")]
    pub logo_beat_glitch_amount: f32,
}

fn default_one() -> f32 {
    1.0
}

fn default_spin_speed() -> f32 {
    0.15
}

fn default_motion_amount() -> f32 {
    0.35
}

fn default_logo_hold() -> f32 {
    8.0
}

fn default_true_stage() -> bool {
    true
}

fn default_disc_color() -> RgbColor {
    RgbColor::new(4.0 / 255.0, 4.0 / 255.0, 6.0 / 255.0)
}

fn default_scope_gain() -> f32 {
    1.15
}

fn default_scope_thickness() -> f32 {
    2.0
}

fn default_scope_color() -> RgbColor {
    RgbColor::new(0.25, 0.95, 1.0)
}

fn default_scope_color_b() -> RgbColor {
    RgbColor::new(1.0, 0.3, 0.85)
}

fn default_spectrum_color() -> RgbColor {
    RgbColor::new(0.35, 1.0, 0.55)
}

fn default_spectrum_color_b() -> RgbColor {
    RgbColor::new(0.2, 0.75, 1.0)
}

fn default_scope_cycle_speed() -> f32 {
    0.2
}

impl Default for StageSettings {
    fn default() -> Self {
        Self {
            background_brightness: 0.85,
            background_darkness: 0.45,
            background_zoom: 1.05,
            background_pan_x: 0.0,
            background_pan_y: 0.0,
            logo_size: 0.72,
            disc_radius: 0.145,
            logo_glow: 0.0,
            bass_pulse: 0.08,
            beat_pulse: 0.14,
            logo_offset_x: 0.0,
            logo_offset_y: 0.0,
            logo_tint_r: 1.0,
            logo_tint_g: 1.0,
            logo_tint_b: 1.0,
            logo_opacity: 1.0,
            logo_brightness: 1.0,
            logo_motion: LogoMotion::None,
            logo_spin_speed: 0.15,
            logo_motion_amount: 0.35,
            disc_ticks: false,
            disc_color: default_disc_color(),
            scope_gain: 1.15,
            scope_thickness: 2.0,
            scope_color_mode: ScopeColorMode::Solid,
            scope_color: default_scope_color(),
            scope_color_b: default_scope_color_b(),
            scope_cycle_speed: 0.2,
            spectrum_gain: 1.15,
            spectrum_thickness: 2.2,
            spectrum_color_mode: ScopeColorMode::Solid,
            spectrum_color: default_spectrum_color(),
            spectrum_color_b: default_spectrum_color_b(),
            spectrum_cycle_speed: 0.15,
            logo_hold_secs: 8.0,
            logo_autoplay: true,
            crt_transition: true,
            logo_glitch: true,
            logo_glitch_amount: 1.0,
            logo_glitch_on_beat: true,
            logo_beat_glitch_amount: 0.85,
        }
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            last_audio_device: None,
            last_preset: Some("default".into()),
            show_debug: true,
            tuning: BandTuning::default(),
            background_path: None,
            logo_path: None,
            logo_paths: Vec::new(),
            logo_styles: Vec::new(),
            stage: StageSettings::default(),
            visualizer: VisualizerSettings::default(),
            particles: ParticleSettings::default(),
            beat: BeatSettings::default(),
            colors: ColorSettings::default(),
        }
    }
}

impl AppSettings {
    /// Fold legacy `logo_path` into `logo_paths` and pad styles.
    pub fn migrate_logo_paths(&mut self) {
        if self.logo_paths.is_empty() {
            if let Some(p) = self.logo_path.take() {
                if !p.is_empty() {
                    self.logo_paths.push(p);
                }
            }
        } else {
            self.logo_path = self.logo_paths.first().cloned();
        }
        self.sync_logo_styles();
    }

    pub fn sync_logo_styles(&mut self) {
        let n = self.logo_paths.len();
        while self.logo_styles.len() < n {
            self.logo_styles.push(LogoStyle::from_stage(&self.stage));
        }
        if self.logo_styles.len() > n {
            self.logo_styles.truncate(n);
        }
    }

    pub fn logo_style(&self, index: usize) -> LogoStyle {
        self.logo_styles
            .get(index)
            .cloned()
            .unwrap_or_else(|| LogoStyle::from_stage(&self.stage))
    }

    pub fn logo_style_mut(&mut self, index: usize) -> &mut LogoStyle {
        self.sync_logo_styles();
        if self.logo_styles.is_empty() {
            self.logo_styles.push(LogoStyle::from_stage(&self.stage));
        }
        let i = index.min(self.logo_styles.len() - 1);
        &mut self.logo_styles[i]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    #[serde(default)]
    pub visualizer: VisualizerSettings,
    #[serde(default)]
    pub particles: ParticleSettings,
    #[serde(default)]
    pub beat: BeatSettings,
    #[serde(default)]
    pub colors: ColorSettings,
    #[serde(default)]
    pub tuning: BandTuning,
    #[serde(default)]
    pub stage: StageSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualizerSettings {
    #[serde(default)]
    pub style: SpectrumStyle,
    #[serde(default)]
    pub color_mode: ColorMode,
    #[serde(default)]
    pub layout: SpectrumLayout,
    pub radius: f32,
    pub thickness: f32,
    pub sensitivity: f32,
    pub smoothing: f32,
    pub bass_response: f32,
    pub mid_response: f32,
    pub high_response: f32,
    pub max_bar_length: f32,
    pub glow: f32,
    pub segment_count: u32,
    /// How tall the mirrored bass “ear” spikes are in Split layout.
    #[serde(default = "default_ear_gain")]
    pub ear_gain: f32,
    /// Number of bass ear lobes per side half → total 2 or 4 around the ring.
    #[serde(default = "default_ear_count")]
    pub ear_count: u8,
    /// Where ears sit along each half (0 = top / 12 o'clock, 1 = bottom).
    #[serde(default = "default_ear_angle")]
    pub ear_angle: f32,
    /// Angular width of each ear lobe.
    #[serde(default = "default_ear_width")]
    pub ear_width: f32,
    /// Extra radial lift for ear spikes (0 = flush with ring base).
    #[serde(default)]
    pub ear_radius: f32,
    /// Outer rim stroke width.
    #[serde(default = "default_outline_width")]
    pub outline_width: f32,
    /// Extra spatial blur on the outer outline (0..1).
    #[serde(default = "default_outline_smooth")]
    pub outline_smooth: f32,
    /// Scroll speed when `color_mode` is Cycle.
    #[serde(default = "default_cycle_speed")]
    pub cycle_speed: f32,
    #[serde(default = "default_min_frequency")]
    pub min_frequency: f32,
    #[serde(default = "default_max_frequency")]
    pub max_frequency: f32,
    #[serde(default = "default_attack_ms")]
    pub attack_ms: f32,
    #[serde(default = "default_release_ms")]
    pub release_ms: f32,
    #[serde(default = "default_noise_floor_db")]
    pub noise_floor_db: f32,
    #[serde(default = "default_ceiling_db")]
    pub ceiling_db: f32,
    #[serde(default = "default_response_exponent")]
    pub response_exponent: f32,
    #[serde(default = "default_frequency_smoothing")]
    pub frequency_smoothing: f32,
    #[serde(default = "default_transient_scale")]
    pub transient_scale: f32,
    /// Turns from 12 o'clock (0.5 = bass at the bottom in Mirrored mode).
    #[serde(default = "default_rotation_offset")]
    pub rotation_offset: f32,
    #[serde(default)]
    pub reverse_spectrum: bool,
    /// Fraction of each angular slot filled by a bar (rest is the gap).
    #[serde(default = "default_bar_fill")]
    pub bar_fill: f32,
    /// Resting bar height as a fraction of the shorter viewport axis.
    #[serde(default = "default_min_bar_height")]
    pub min_bar_height: f32,
    #[serde(default)]
    pub show_base_ring: bool,
    #[serde(default = "default_base_ring_width")]
    pub base_ring_width: f32,
    #[serde(default = "default_base_ring_brightness")]
    pub base_ring_brightness: f32,
    #[serde(default)]
    pub bar_cap: BarCap,
    #[serde(default)]
    pub show_peak_caps: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticleSettings {
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub count: u32,
    pub size: f32,
    pub speed: f32,
    pub gravity: f32,
    pub opacity: f32,
    pub lifetime: f32,
    pub bass_reaction: f32,
    pub beat_reaction: f32,
    pub spread: f32,
    #[serde(default)]
    pub glow: f32,
    /// Independent from the spectrum ring.
    #[serde(default)]
    pub color_mode: ColorMode,
    #[serde(default = "default_particle_solid")]
    pub solid: RgbColor,
    #[serde(default = "default_particle_gradient_low")]
    pub gradient_low: RgbColor,
    #[serde(default = "default_particle_gradient_high")]
    pub gradient_high: RgbColor,
    #[serde(default = "default_cycle_speed")]
    pub cycle_speed: f32,
}

fn default_true() -> bool {
    true
}

fn default_ear_gain() -> f32 {
    1.45
}
fn default_ear_count() -> u8 {
    2
}
fn default_ear_angle() -> f32 {
    0.20
}
fn default_ear_width() -> f32 {
    0.075
}
fn default_outline_width() -> f32 {
    1.45
}
fn default_outline_smooth() -> f32 {
    0.45
}
fn default_cycle_speed() -> f32 {
    0.18
}
fn default_min_frequency() -> f32 {
    30.0
}
fn default_max_frequency() -> f32 {
    16_000.0
}
fn default_attack_ms() -> f32 {
    28.0
}
fn default_release_ms() -> f32 {
    210.0
}
fn default_noise_floor_db() -> f32 {
    -66.0
}
fn default_ceiling_db() -> f32 {
    -14.0
}
fn default_response_exponent() -> f32 {
    1.7
}
fn default_frequency_smoothing() -> f32 {
    0.34
}
fn default_transient_scale() -> f32 {
    0.12
}
fn default_rotation_offset() -> f32 {
    0.5
}
fn default_bar_fill() -> f32 {
    0.74
}
fn default_min_bar_height() -> f32 {
    0.008
}
fn default_base_ring_width() -> f32 {
    1.15
}
fn default_base_ring_brightness() -> f32 {
    0.16
}
fn default_particle_solid() -> RgbColor {
    RgbColor::new(0.35, 0.85, 1.0)
}
fn default_particle_gradient_low() -> RgbColor {
    RgbColor::new(1.0, 0.35, 0.55)
}
fn default_particle_gradient_high() -> RgbColor {
    RgbColor::new(0.25, 0.9, 1.0)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeatSettings {
    pub sensitivity: f32,
    pub cooldown: f32,
    pub kick_sensitivity: f32,
    #[serde(default)]
    pub mode: BeatMode,
    #[serde(default = "default_bass_weight")]
    pub bass_weight: f32,
    #[serde(default = "default_rms_weight")]
    pub rms_weight: f32,
}

fn default_bass_weight() -> f32 {
    1.0
}
fn default_rms_weight() -> f32 {
    0.35
}

/// Linear RGB in 0..1 (may exceed 1 for boosted logo tints elsewhere).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct RgbColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl RgbColor {
    pub const fn new(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b }
    }

    pub fn to_srgb(self) -> [u8; 3] {
        [
            (self.r.clamp(0.0, 1.0) * 255.0) as u8,
            (self.g.clamp(0.0, 1.0) * 255.0) as u8,
            (self.b.clamp(0.0, 1.0) * 255.0) as u8,
        ]
    }

    pub fn from_srgb(c: [u8; 3]) -> Self {
        Self {
            r: c[0] as f32 / 255.0,
            g: c[1] as f32 / 255.0,
            b: c[2] as f32 / 255.0,
        }
    }

    pub fn lerp(self, other: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Self {
            r: self.r + (other.r - self.r) * t,
            g: self.g + (other.g - self.g) * t,
            b: self.b + (other.b - self.b) * t,
        }
    }

    pub fn scale(self, v: f32) -> Self {
        Self {
            r: (self.r * v).clamp(0.0, 1.0),
            g: (self.g * v).clamp(0.0, 1.0),
            b: (self.b * v).clamp(0.0, 1.0),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorSettings {
    pub rgb_intensity: f32,
    pub glow_intensity: f32,
    /// Used when `color_mode` is Solid (and legacy Cyan/Amber/Magenta overrides).
    #[serde(default = "default_solid_color")]
    pub solid: RgbColor,
    #[serde(default = "default_gradient_low")]
    pub gradient_low: RgbColor,
    #[serde(default = "default_gradient_high")]
    pub gradient_high: RgbColor,
    #[serde(default = "default_band_bass")]
    pub band_bass: RgbColor,
    #[serde(default = "default_band_low_mid")]
    pub band_low_mid: RgbColor,
    #[serde(default = "default_band_mid")]
    pub band_mid: RgbColor,
    #[serde(default = "default_band_high_mid")]
    pub band_high_mid: RgbColor,
    #[serde(default = "default_band_high")]
    pub band_high: RgbColor,
}

fn default_solid_color() -> RgbColor {
    RgbColor::new(0.18, 0.95, 1.0)
}
fn default_gradient_low() -> RgbColor {
    RgbColor::new(0.18, 0.95, 1.0)
}
fn default_gradient_high() -> RgbColor {
    RgbColor::new(1.0, 0.32, 0.72)
}
fn default_band_bass() -> RgbColor {
    RgbColor::new(0.18, 0.95, 1.0)
}
fn default_band_low_mid() -> RgbColor {
    RgbColor::new(0.22, 0.50, 1.0)
}
fn default_band_mid() -> RgbColor {
    RgbColor::new(0.62, 0.22, 1.0)
}
fn default_band_high_mid() -> RgbColor {
    RgbColor::new(0.98, 0.16, 0.90)
}
fn default_band_high() -> RgbColor {
    RgbColor::new(1.0, 0.40, 0.62)
}

impl Default for VisualizerSettings {
    fn default() -> Self {
        Self {
            style: SpectrumStyle::Bars,
            color_mode: ColorMode::Rainbow,
            layout: SpectrumLayout::Mirrored,
            radius: 0.22,
            thickness: 2.0,
            sensitivity: 1.25,
            smoothing: 0.40,
            bass_response: 0.95,
            mid_response: 1.12,
            high_response: 0.88,
            max_bar_length: 0.36,
            glow: 0.48,
            segment_count: 192,
            ear_gain: 1.45,
            ear_count: 2,
            ear_angle: 0.20,
            ear_width: 0.075,
            ear_radius: 0.06,
            outline_width: 1.40,
            outline_smooth: 0.40,
            cycle_speed: 0.15,
            min_frequency: 30.0,
            max_frequency: 16_000.0,
            attack_ms: 28.0,
            release_ms: 210.0,
            noise_floor_db: -66.0,
            ceiling_db: -14.0,
            response_exponent: 1.7,
            frequency_smoothing: 0.34,
            transient_scale: 0.12,
            rotation_offset: 0.5,
            reverse_spectrum: false,
            bar_fill: 0.74,
            min_bar_height: 0.008,
            show_base_ring: false,
            base_ring_width: 1.15,
            base_ring_brightness: 0.16,
            bar_cap: BarCap::Square,
            show_peak_caps: false,
        }
    }
}

impl VisualizerSettings {
    pub fn analysis_config(&self, tuning: &BandTuning) -> crate::audio::SpectrumAnalysisConfig {
        crate::audio::SpectrumAnalysisConfig::from_visualizer(
            self.segment_count,
            self.min_frequency,
            self.max_frequency,
            self.attack_ms,
            self.release_ms,
            self.noise_floor_db,
            self.ceiling_db,
            self.response_exponent,
            self.frequency_smoothing,
            self.bass_response,
            self.mid_response,
            self.high_response,
            self.transient_scale,
            self.sensitivity * tuning.sensitivity,
        )
    }
}

impl Default for ParticleSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            count: 1000,
            size: 2.0,
            speed: 0.4,
            gravity: 0.0,
            opacity: 0.7,
            lifetime: 2.5,
            bass_reaction: 1.0,
            beat_reaction: 1.2,
            spread: 0.8,
            glow: 0.4,
            color_mode: ColorMode::Rainbow,
            solid: default_particle_solid(),
            gradient_low: default_particle_gradient_low(),
            gradient_high: default_particle_gradient_high(),
            cycle_speed: 0.18,
        }
    }
}

impl Default for BeatSettings {
    fn default() -> Self {
        Self {
            sensitivity: 0.85,
            cooldown: 0.22,
            kick_sensitivity: 1.0,
            mode: BeatMode::KickOnly,
            bass_weight: 1.0,
            rms_weight: 0.0,
        }
    }
}

impl BeatSettings {
    pub fn preset_bass_only() -> Self {
        Self {
            sensitivity: 1.05,
            cooldown: 0.18,
            kick_sensitivity: 1.05,
            mode: BeatMode::BassKick,
            bass_weight: 1.0,
            rms_weight: 0.0,
        }
    }

    pub fn preset_kick_only() -> Self {
        Self {
            sensitivity: 0.85,
            cooldown: 0.22,
            kick_sensitivity: 1.1,
            mode: BeatMode::KickOnly,
            bass_weight: 1.0,
            rms_weight: 0.0,
        }
    }

    pub fn preset_bass_level() -> Self {
        Self {
            sensitivity: 1.15,
            cooldown: 0.16,
            kick_sensitivity: 0.9,
            mode: BeatMode::BassLevel,
            bass_weight: 1.1,
            rms_weight: 0.0,
        }
    }

    pub fn preset_sensitive() -> Self {
        Self {
            sensitivity: 1.35,
            cooldown: 0.16,
            kick_sensitivity: 1.2,
            mode: BeatMode::BassKick,
            bass_weight: 1.0,
            rms_weight: 0.0,
        }
    }

    pub fn preset_tight() -> Self {
        Self {
            sensitivity: 0.75,
            cooldown: 0.26,
            kick_sensitivity: 1.0,
            mode: BeatMode::KickOnly,
            bass_weight: 1.0,
            rms_weight: 0.0,
        }
    }
}

impl Default for ColorSettings {
    fn default() -> Self {
        Self {
            rgb_intensity: 1.08,
            glow_intensity: 0.5,
            solid: default_solid_color(),
            gradient_low: default_gradient_low(),
            gradient_high: default_gradient_high(),
            band_bass: default_band_bass(),
            band_low_mid: default_band_low_mid(),
            band_mid: default_band_mid(),
            band_high_mid: default_band_high_mid(),
            band_high: default_band_high(),
        }
    }
}

impl ColorSettings {
    /// Sample a color for spectrum/FFT position `t` in 0..1 (bass → high).
    pub fn sample(&self, mode: ColorMode, t: f32, intensity: f32) -> RgbColor {
        self.sample_cycled(mode, t, intensity, 0.0)
    }

    /// Like [`sample`](Self::sample), with a time offset for `ColorMode::Cycle`.
    pub     fn sample_cycled(
        &self,
        mode: ColorMode,
        t: f32,
        intensity: f32,
        cycle: f32,
    ) -> RgbColor {
        let boost = (intensity * self.rgb_intensity).clamp(0.35, 1.35);
        let t = t.rem_euclid(1.0);
        match mode {
            ColorMode::Rainbow => {
                let (r, g, b) = neon_ring_gradient(t);
                RgbColor::new(r, g, b).scale(boost.min(1.0))
            }
            ColorMode::Cycle => {
                let (r, g, b) = neon_ring_gradient((t + cycle).rem_euclid(1.0));
                RgbColor::new(r, g, b).scale(boost.min(1.0))
            }
            ColorMode::Solid => self.solid.scale(boost.min(1.0)),
            ColorMode::Gradient => self
                .gradient_low
                .lerp(self.gradient_high, t)
                .scale(boost.min(1.0)),
            ColorMode::Bands => self.sample_bands(t).scale(boost.min(1.0)),
            ColorMode::Mono => {
                let g = (0.58 + 0.38 * intensity.clamp(0.0, 1.0)).min(1.0);
                RgbColor::new(g, g, g)
            }
            ColorMode::Cyan => RgbColor::new(0.2, 0.82, 1.0).scale(boost.min(1.0)),
            ColorMode::Amber => RgbColor::new(1.0, 0.67, 0.22).scale(boost.min(1.0)),
            ColorMode::Magenta => RgbColor::new(1.0, 0.22, 0.75).scale(boost.min(1.0)),
        }
    }

    fn sample_bands(&self, t: f32) -> RgbColor {
        let stops = [
            (0.0, self.band_bass),
            (0.22, self.band_low_mid),
            (0.45, self.band_mid),
            (0.7, self.band_high_mid),
            (1.0, self.band_high),
        ];
        for w in stops.windows(2) {
            let (t0, c0) = w[0];
            let (t1, c1) = w[1];
            if t <= t1 + f32::EPSILON {
                let u = if (t1 - t0).abs() < f32::EPSILON {
                    0.0
                } else {
                    ((t - t0) / (t1 - t0)).clamp(0.0, 1.0)
                };
                let u = u * u * (3.0 - 2.0 * u);
                return c0.lerp(c1, u);
            }
        }
        self.band_high
    }
}

/// Frequency neon: cyan → electric blue → violet → magenta → pink.
/// `t` is 0 at bass and 1 at highs (does not wrap back to cyan).
pub fn neon_ring_gradient(t: f32) -> (f32, f32, f32) {
    const STOPS: [(f32, f32, f32, f32); 5] = [
        (0.00, 0.18, 0.98, 1.00),
        (0.24, 0.20, 0.50, 1.00),
        (0.48, 0.62, 0.22, 1.00),
        (0.74, 0.98, 0.16, 0.90),
        (1.00, 1.00, 0.40, 0.62),
    ];
    sample_positioned_stops(t, &STOPS)
}

fn sample_positioned_stops(t: f32, stops: &[(f32, f32, f32, f32)]) -> (f32, f32, f32) {
    let t = t.clamp(0.0, 1.0);
    if stops.is_empty() {
        return (1.0, 1.0, 1.0);
    }
    if t <= stops[0].0 {
        return (stops[0].1, stops[0].2, stops[0].3);
    }
    for w in stops.windows(2) {
        let a = w[0];
        let b = w[1];
        if t <= b.0 {
            let span = (b.0 - a.0).max(1e-5);
            let mut u = ((t - a.0) / span).clamp(0.0, 1.0);
            u = u * u * (3.0 - 2.0 * u);
            return (
                a.1 + (b.1 - a.1) * u,
                a.2 + (b.2 - a.2) * u,
                a.3 + (b.3 - a.3) * u,
            );
        }
    }
    let last = stops[stops.len() - 1];
    (last.1, last.2, last.3)
}

/// Neon rainbow stops shared by particles / FFT extras.
pub fn neon_rainbow(t: f32) -> (f32, f32, f32) {
    const STOPS: [(f32, f32, f32); 6] = [
        (1.00, 0.20, 0.35),
        (1.00, 0.75, 0.15),
        (0.25, 1.00, 0.40),
        (0.15, 0.90, 1.00),
        (0.35, 0.35, 1.00),
        (0.95, 0.25, 0.95),
    ];
    let n = STOPS.len() as f32;
    let x = t.fract() * n;
    let i = x.floor() as usize % STOPS.len();
    let j = (i + 1) % STOPS.len();
    let f = x - x.floor();
    let f = (1.0 - (f * std::f32::consts::PI).cos()) * 0.5;
    let a = STOPS[i];
    let b = STOPS[j];
    (
        a.0 + (b.0 - a.0) * f,
        a.1 + (b.1 - a.1) * f,
        a.2 + (b.2 - a.2) * f,
    )
}

impl Default for Preset {
    fn default() -> Self {
        Self {
            name: "default".into(),
            visualizer: VisualizerSettings::default(),
            particles: ParticleSettings::default(),
            beat: BeatSettings::default(),
            colors: ColorSettings::default(),
            tuning: BandTuning::default(),
            stage: StageSettings::default(),
        }
    }
}

pub fn settings_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("live_visualizer")
        .join("settings.toml")
}

/// User-writable presets — always under the app config dir (reliable regardless of cwd).
pub fn user_presets_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("live_visualizer")
        .join("presets")
}

/// Directory used for new saves (user presets).
pub fn presets_dir() -> PathBuf {
    user_presets_dir()
}

/// Built-in factory presets (compiled into the binary from `presets/*.toml`).
fn builtin_preset_toml(name: &str) -> Option<&'static str> {
    match name {
        "default" => Some(include_str!("../../presets/default.toml")),
        "rgb" => Some(include_str!("../../presets/rgb.toml")),
        "mono" => Some(include_str!("../../presets/mono.toml")),
        "smooth" => Some(include_str!("../../presets/smooth.toml")),
        "particles" => Some(include_str!("../../presets/particles.toml")),
        "bar" => Some(include_str!("../../presets/bar.toml")),
        "neon" => Some(include_str!("../../presets/neon.toml")),
        "beat_bass" => Some(include_str!("../../presets/beat_bass.toml")),
        "beat_kick" => Some(include_str!("../../presets/beat_kick.toml")),
        _ => None,
    }
}

fn builtin_preset_names() -> &'static [&'static str] {
    &[
        "bar",
        "beat_bass",
        "beat_kick",
        "default",
        "mono",
        "neon",
        "particles",
        "rgb",
        "smooth",
    ]
}

/// Built-in factory preset name. Cannot be overwritten by user saves.
pub const FACTORY_PRESET_NAME: &str = "default";

pub fn is_locked_preset(name: &str) -> bool {
    sanitize_preset_name(name).eq_ignore_ascii_case(FACTORY_PRESET_NAME)
}

/// Recommended look + tuning. Source of truth is the Rust `Default` impls.
pub fn factory_preset() -> Preset {
    Preset::default()
}
pub fn sanitize_preset_name(name: &str) -> String {
    let trimmed = name.trim();
    let mut out = String::with_capacity(trimmed.len());
    for ch in trimmed.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else if ch.is_whitespace() {
            if !out.ends_with('_') {
                out.push('_');
            }
        }
    }
    let out = out.trim_matches('_').to_string();
    if out.is_empty() {
        "custom".into()
    } else {
        out
    }
}

fn collect_preset_names(dir: &PathBuf, into: &mut Vec<String>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("toml") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if !into.iter().any(|n| n == stem) {
                        into.push(stem.to_string());
                    }
                }
            }
        }
    }
}

pub fn load_settings() -> Result<AppSettings> {
    let path = settings_path();
    if !path.exists() {
        return Ok(AppSettings::default());
    }
    let text = fs::read_to_string(&path).with_context(|| format!("Reading {path:?}"))?;
    match toml::from_str::<AppSettings>(&text) {
        Ok(mut settings) => {
            settings.migrate_logo_paths();
            Ok(settings)
        }
        Err(e) => {
            log::warn!("Corrupt settings.toml ({e}); backing up and using defaults");
            let bak = path.with_extension("toml.bak");
            let _ = fs::rename(&path, &bak);
            Ok(AppSettings::default())
        }
    }
}

pub fn save_settings(settings: &AppSettings) -> Result<()> {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = toml::to_string_pretty(settings)?;
    // Atomic-ish write to avoid corruption if killed mid-save
    let tmp = path.with_extension("toml.tmp");
    fs::write(&tmp, text).with_context(|| format!("Writing {tmp:?}"))?;
    fs::rename(&tmp, &path).with_context(|| format!("Renaming {tmp:?} -> {path:?}"))?;
    Ok(())
}

pub fn list_presets() -> Vec<String> {
    let mut names: Vec<String> = builtin_preset_names()
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    collect_preset_names(&user_presets_dir(), &mut names);
    names.retain(|n| !is_locked_preset(n));
    names.sort();
    names.dedup();
    let mut out = vec![FACTORY_PRESET_NAME.to_string()];
    out.extend(names);
    out
}

pub fn load_preset(name: &str) -> Result<Preset> {
    if is_locked_preset(name) {
        return Ok(factory_preset());
    }
    // User saves override built-ins of the same name.
    let user_path = user_presets_dir().join(format!("{name}.toml"));
    let text = if user_path.exists() {
        fs::read_to_string(&user_path).with_context(|| format!("Reading {user_path:?}"))?
    } else if let Some(builtin) = builtin_preset_toml(name) {
        builtin.to_string()
    } else {
        anyhow::bail!("Unknown preset '{name}'");
    };
    let mut preset: Preset = toml::from_str(&text).context("Parsing preset")?;
    if preset.name.is_empty() {
        preset.name = name.to_string();
    }
    Ok(preset)
}

pub fn save_preset(preset: &Preset) -> Result<PathBuf> {
    let safe = sanitize_preset_name(&preset.name);
    if is_locked_preset(&safe) {
        anyhow::bail!("'{FACTORY_PRESET_NAME}' is a built-in factory preset and cannot be overwritten");
    }
    let dir = user_presets_dir();
    fs::create_dir_all(&dir)
        .with_context(|| format!("Creating presets folder {dir:?}"))?;
    let path = dir.join(format!("{safe}.toml"));
    let mut to_write = preset.clone();
    to_write.name = safe;
    let text = toml::to_string_pretty(&to_write).context("Serializing preset")?;
    let tmp = path.with_extension("toml.tmp");
    fs::write(&tmp, text).with_context(|| format!("Writing {tmp:?}"))?;
    fs::rename(&tmp, &path).with_context(|| format!("Renaming {tmp:?} -> {path:?}"))?;
    Ok(path)
}

impl AppSettings {
    pub fn apply_preset(&mut self, preset: &Preset) {
        self.visualizer = preset.visualizer.clone();
        self.particles = preset.particles.clone();
        self.beat = preset.beat.clone();
        self.colors = preset.colors.clone();
        self.tuning = preset.tuning.clone();
        // Keep media paths; take stage look from preset
        self.stage = preset.stage.clone();
        self.last_preset = Some(preset.name.clone());
    }

    pub fn to_preset(&self, name: &str) -> Preset {
        Preset {
            name: name.to_string(),
            visualizer: self.visualizer.clone(),
            particles: self.particles.clone(),
            beat: self.beat.clone(),
            colors: self.colors.clone(),
            tuning: self.tuning.clone(),
            stage: self.stage.clone(),
        }
    }
}
