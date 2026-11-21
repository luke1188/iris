//! Settings persistence and presets.

#![allow(unused_imports)]

mod preset;

pub use preset::{
    center_item_label, factory_preset, is_center_effect, is_center_scope, is_center_spectrum,
    is_locked_preset, is_template, list_presets, list_templates, list_user_presets, load_preset,
    load_settings, load_template, neon_rainbow, neon_ring_gradient, presets_dir,
    sanitize_preset_name, save_preset, save_settings, settings_path, template_label,
    user_presets_dir, AppSettings, BannerCorner, BannerImageFloat, BannerItem, BannerSettings,
    BarCap, BeatSettings,
    ColorMode, ColorSettings, LogoMotion, LogoStyle, LogoTransition, ParticleSettings, Preset,
    RgbColor, ScopeColorMode, SpectrumLayout, SpectrumStyle, StageSettings, VisualizerSettings,
    CENTER_SCOPE_ID, CENTER_SPECTRUM_ID, FACTORY_PRESET_NAME, TEMPLATE_NAMES,
};

pub use crate::audio::BeatMode;
