//! Settings persistence and presets.

#![allow(unused_imports)]

mod preset;

pub use preset::{
    bundled_presets_dir, list_presets, load_preset, load_settings, presets_dir, sanitize_preset_name,
    save_preset, save_settings, settings_path, user_presets_dir, AppSettings, BeatSettings,
    ColorMode, ColorSettings, LogoMotion, ParticleSettings, Preset, SpectrumLayout, SpectrumStyle,
    StageSettings, VisualizerSettings,
};

pub use crate::audio::BeatMode;
