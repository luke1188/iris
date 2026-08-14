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
    #[default]
    Rgb,
    Mono,
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
    /// Full circle, bass → highs.
    #[default]
    Full,
    /// Lows / mids / highs mirrored on the left and right (no bass on the ring).
    Split,
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
    /// Seconds each logo stays before cycling (multi-logo).
    #[serde(default = "default_logo_hold")]
    pub logo_hold_secs: f32,
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

impl Default for StageSettings {
    fn default() -> Self {
        Self {
            background_brightness: 0.85,
            background_darkness: 0.45,
            background_zoom: 1.05,
            background_pan_x: 0.0,
            background_pan_y: 0.0,
            logo_size: 0.72,
            disc_radius: 0.14,
            logo_glow: 0.0,
            bass_pulse: 0.1,
            beat_pulse: 0.18,
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
            logo_hold_secs: 8.0,
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
            last_preset: Some("rgb".into()),
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
}

fn default_true() -> bool {
    true
}

fn default_ear_gain() -> f32 {
    1.45
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorSettings {
    pub rgb_intensity: f32,
    pub glow_intensity: f32,
}

impl Default for VisualizerSettings {
    fn default() -> Self {
        Self {
            style: SpectrumStyle::Bars,
            color_mode: ColorMode::Rgb,
            layout: SpectrumLayout::Full,
            radius: 0.22,
            thickness: 2.0,
            sensitivity: 1.4,
            smoothing: 0.28,
            bass_response: 1.2,
            mid_response: 1.0,
            high_response: 0.9,
            max_bar_length: 0.55,
            glow: 0.5,
            segment_count: 256,
            ear_gain: 1.45,
        }
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
            rgb_intensity: 1.0,
            glow_intensity: 0.6,
        }
    }
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

/// Factory / bundled presets (repo USB copy or next to the exe).
pub fn bundled_presets_dir() -> PathBuf {
    let local = PathBuf::from("presets");
    if local.is_dir() {
        return local;
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let beside = parent.join("presets");
            if beside.is_dir() {
                return beside;
            }
        }
    }
    local
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

/// Safe file stem for a preset name (spaces → `_`, strip unsafe chars).
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
    let mut names = Vec::new();
    // User presets first so they win on name collisions in the list order sense;
    // load_preset also prefers user.
    collect_preset_names(&user_presets_dir(), &mut names);
    collect_preset_names(&bundled_presets_dir(), &mut names);
    names.sort();
    names
}

fn preset_path(name: &str) -> PathBuf {
    let file = format!("{name}.toml");
    let user = user_presets_dir().join(&file);
    if user.exists() {
        return user;
    }
    bundled_presets_dir().join(file)
}

pub fn load_preset(name: &str) -> Result<Preset> {
    let path = preset_path(name);
    let text = fs::read_to_string(&path).with_context(|| format!("Reading {path:?}"))?;
    let mut preset: Preset = toml::from_str(&text).context("Parsing preset")?;
    if preset.name.is_empty() {
        preset.name = name.to_string();
    }
    Ok(preset)
}

pub fn save_preset(preset: &Preset) -> Result<PathBuf> {
    let dir = user_presets_dir();
    fs::create_dir_all(&dir)
        .with_context(|| format!("Creating presets folder {dir:?}"))?;
    let safe = sanitize_preset_name(&preset.name);
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
