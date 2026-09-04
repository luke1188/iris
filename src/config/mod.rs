//! Settings persistence and presets.

#![allow(unused_imports)]

mod preset;

pub use preset::{
    center_item_label, is_center_effect, is_center_scope, is_center_spectrum, list_presets,
    load_preset, load_settings, neon_rainbow, presets_dir, sanitize_preset_name, save_preset,
    save_settings, settings_path, user_presets_dir, AppSettings, BeatSettings, CENTER_SCOPE_ID,
    CENTER_SPECTRUM_ID, ColorMode, ColorSettings, LogoMotion, LogoStyle, ParticleSettings, Preset,
    RgbColor, ScopeColorMode, SpectrumLayout, SpectrumStyle, StageSettings, VisualizerSettings,
};

pub use crate::audio::BeatMode;
