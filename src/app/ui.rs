//! egui settings panel and stage preview.

use super::media::{cover_rect, fit_rect};
use super::state::LiveVisualizerApp;
use crate::config::{
    BeatSettings, ColorMode, ColorSettings, LogoMotion, RgbColor, SpectrumLayout, SpectrumStyle,
};
use egui::epaint::Mesh;
use egui::{self, Color32, Pos2, RichText, Sense, Shape, Stroke};

pub fn draw_settings_panel(ctx: &egui::Context, app: &mut LiveVisualizerApp) {
    egui::SidePanel::left("settings")
        .default_width(350.0)
        .min_width(300.0)
        .resizable(true)
        .show(ctx, |ui| {
            draw_settings_contents(ui, app);
        });
}

/// Shared settings UI for docked side panel and detached popout window.
pub fn draw_settings_contents(ui: &mut egui::Ui, app: &mut LiveVisualizerApp) {
    ui.horizontal(|ui| {
        ui.heading(RichText::new("Iris Visualizer").color(Color32::from_rgb(0, 220, 255)));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let pop = if app.settings_popout { "Dock" } else { "↗ Pop out" };
            if ui
                .button(pop)
                .on_hover_text("Open on another monitor while the stage stays fullscreen")
                .clicked()
            {
                app.settings_popout = !app.settings_popout;
                if app.settings_popout {
                    app.show_settings = true;
                }
            }
        });
    });

    if let Some(ref err) = app.error_message.clone() {
        ui.colored_label(Color32::from_rgb(255, 80, 80), err);
    }
    if let Some(ref ok) = app.status_message.clone() {
        ui.colored_label(Color32::from_rgb(80, 220, 140), ok);
    }

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        for (i, label) in ["Live", "Look", "Tune", "More"].iter().enumerate() {
            let selected = app.controls_tab == i;
            if ui
                .selectable_label(selected, RichText::new(*label).strong())
                .clicked()
            {
                app.controls_tab = i;
            }
        }
    });
    ui.separator();

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            match app.controls_tab {
                0 => {
                    section_hint(ui, "Pick an input, watch levels, tune the bounce");
                    draw_audio_section(ui, app);
                    ui.add_space(8.0);
                    draw_levels_section(ui, app);
                    ui.add_space(8.0);
                    draw_beat_section(ui, app);
                }
                1 => {
                    section_hint(ui, "Logo, background, and spectrum look");
                    draw_logo_section(ui, app);
                    ui.add_space(8.0);
                    draw_background_section(ui, app);
                    ui.add_space(8.0);
                    draw_visualizer_section(ui, app);
                }
                2 => {
                    section_hint(ui, "Gain & band balance for the room");
                    draw_tuning_section(ui, app);
                }
                _ => {
                    section_hint(ui, "Presets, particles, and display");
                    draw_presets_section(ui, app);
                    ui.add_space(8.0);
                    draw_particles_section(ui, app);
                    ui.add_space(8.0);
                    draw_display_section(ui, app);
                }
            }
            ui.add_space(12.0);
            ui.label(
                RichText::new("F11 fullscreen · Esc exit FS · F1 toggle UI")
                    .small()
                    .color(Color32::DARK_GRAY),
            );
        });
}

fn section_hint(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).small().color(Color32::GRAY));
    ui.add_space(4.0);
}

fn section_frame(ui: &mut egui::Ui, title: &str, default_open: bool, add_contents: impl FnOnce(&mut egui::Ui)) {
    egui::CollapsingHeader::new(RichText::new(title).strong())
        .default_open(default_open)
        .show(ui, |ui| {
            ui.add_space(2.0);
            add_contents(ui);
        });
}

fn draw_presets_section(ui: &mut egui::Ui, app: &mut LiveVisualizerApp) {
    section_frame(ui, "Presets", true, |ui| {
        ui.horizontal(|ui| {
            if ui.button("Refresh").on_hover_text("Reload user presets folder").clicked() {
                app.refresh_presets();
            }
            let active = match (
                app.settings.last_preset.as_deref(),
                app.preset_dirty,
            ) {
                (Some(name), true) => format!("{name} · edited"),
                (Some(name), false) => name.to_string(),
                (None, true) => "session · unsaved".into(),
                (None, false) => "none".into(),
            };
            ui.label(
                RichText::new(format!("Active: {active}"))
                    .small()
                    .color(if app.preset_dirty {
                        Color32::from_rgb(255, 190, 80)
                    } else {
                        Color32::GRAY
                    }),
            );
        });
        ui.label(
            RichText::new(format!(
                "Session autosaves · presets only when you click Save → {}",
                crate::config::presets_dir().display()
            ))
            .small()
            .color(Color32::DARK_GRAY),
        );

        ui.add_space(4.0);
        let names = app.preset_names.clone();
        let pending = app.pending_preset_load.clone();
        egui::Grid::new("preset_grid")
            .num_columns(2)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                for name in &names {
                    let selected = !app.preset_dirty
                        && app.settings.last_preset.as_deref() == Some(name.as_str());
                    let label = if pending.as_deref() == Some(name.as_str()) {
                        format!("{name}  (click again)")
                    } else {
                        name.clone()
                    };
                    if ui.selectable_label(selected, label).clicked() {
                        app.apply_preset_by_name(name);
                    }
                    ui.end_row();
                }
            });

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Save as");
            ui.text_edit_singleline(&mut app.preset_save_name);
            let save_label = if app.preset_names.iter().any(|n| n == &app.preset_save_name) {
                "Overwrite"
            } else {
                "Save"
            };
            if ui
                .button(save_label)
                .on_hover_text("Writes a named preset file (session already autosaves)")
                .clicked()
            {
                app.save_current_preset();
            }
        });
        ui.label(
            RichText::new("Letters, numbers, - and _ only (spaces become _)")
                .small()
                .color(Color32::DARK_GRAY),
        );
    });
}

fn draw_beat_section(ui: &mut egui::Ui, app: &mut LiveVisualizerApp) {
    section_frame(ui, "Beat / bounce", true, |ui| {
        level_bar(ui, "Beat", app.features.beat, Color32::from_rgb(255, 100, 255));
        level_bar(ui, "Kick", app.features.kick, Color32::from_rgb(255, 70, 140));
        ui.add_space(6.0);

        ui.label(RichText::new("Quick setups").small().strong());
        ui.horizontal_wrapped(|ui| {
            if ui
                .button("Bass kick")
                .on_hover_text("Kick hits with bass weight — good default for most tracks")
                .clicked()
            {
                app.apply_beat_preset(BeatSettings::preset_bass_only());
            }
            if ui
                .button("Kick only")
                .on_hover_text("Strict drum kicks — ignores sustained 808 rumble")
                .clicked()
            {
                app.apply_beat_preset(BeatSettings::preset_kick_only());
            }
            if ui
                .button("Bass level")
                .on_hover_text("Follows bass energy (sustains + kicks) — great for 808s")
                .clicked()
            {
                app.apply_beat_preset(BeatSettings::preset_bass_level());
            }
            if ui
                .button("Sensitive")
                .on_hover_text("Fires more easily — quieter rooms / soft kicks")
                .clicked()
            {
                app.apply_beat_preset(BeatSettings::preset_sensitive());
            }
            if ui
                .button("Tight")
                .on_hover_text("Fewer hits, longer cooldown — busy tracks / loud clubs")
                .clicked()
            {
                app.apply_beat_preset(BeatSettings::preset_tight());
            }
        });

        ui.add_space(6.0);
        let mut changed = false;
        {
            let b = &mut app.settings.beat;
            changed |= slider(ui, "Sensitivity", &mut b.sensitivity, 0.4..=3.5)
                .on_hover_text("Overall beat threshold")
                .changed();
            changed |= slider(ui, "Kick punch", &mut b.kick_sensitivity, 0.3..=3.0)
                .on_hover_text("How hard kick attacks need to hit")
                .changed();
            changed |= slider(ui, "Bass weight", &mut b.bass_weight, 0.0..=2.5)
                .on_hover_text("How much bass energy feeds the bounce")
                .changed();
            changed |= slider(ui, "RMS weight", &mut b.rms_weight, 0.0..=2.0)
                .on_hover_text("Overall loudness contribution (usually leave low)")
                .changed();
            changed |= slider(ui, "Cooldown", &mut b.cooldown, 0.16..=0.4)
                .on_hover_text("Minimum time between beat pulses")
                .changed();
        }

        if ui
            .button("Reset disc bounce")
            .on_hover_text("Restores Bass bounce / Beat bounce on the logo disc")
            .clicked()
        {
            app.settings.stage.bass_pulse = 0.12;
            app.settings.stage.beat_pulse = 0.22;
            app.mark_settings_dirty();
        }

        if changed {
            app.apply_beat_from_ui();
        }
    });
}

fn draw_audio_section(ui: &mut egui::Ui, app: &mut LiveVisualizerApp) {
    section_frame(ui, "Audio input", true, |ui| {
        ui.horizontal(|ui| {
            ui.label("Device");
            if ui.button("Refresh").clicked() {
                app.refresh_devices();
            }
        });

        let current = app
            .selected_device
            .clone()
            .unwrap_or_else(|| "(none)".into());

        egui::ComboBox::from_id_salt("audio_device")
            .selected_text(&current)
            .width(ui.available_width() - 8.0)
            .show_ui(ui, |ui| {
                for device in &app.devices.clone() {
                    let selected = app.selected_device.as_deref() == Some(device.id.as_str());
                    let label = if device.is_loopback {
                        format!("{} [loopback]", device.name)
                    } else {
                        device.name.clone()
                    };
                    if ui.selectable_label(selected, &label).clicked() {
                        let id = device.id.clone();
                        app.select_device(&id);
                    }
                }
            });

        if let Some(sr) = app.capture.sample_rate() {
            ui.label(
                RichText::new(format!("{sr} Hz · {} ch", app.capture.channels().unwrap_or(1)))
                    .small()
                    .color(Color32::GRAY),
            );
        }

        ui.add_space(6.0);
        ui.label(RichText::new("Input level").small().strong());
        let level = app.features.rms.clamp(0.0, 1.0);
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 14.0), Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(rect, 2.0, Color32::from_rgb(30, 30, 40));
        let fill =
            egui::Rect::from_min_size(rect.min, egui::vec2(rect.width() * level, rect.height()));
        let color = if level > 0.85 {
            Color32::from_rgb(255, 60, 60)
        } else if level > 0.6 {
            Color32::from_rgb(255, 200, 40)
        } else {
            Color32::from_rgb(0, 220, 160)
        };
        painter.rect_filled(fill, 2.0, color);
        ui.checkbox(&mut app.show_fft_debug, "Show debug FFT graph");
    });
}

fn draw_tuning_section(ui: &mut egui::Ui, app: &mut LiveVisualizerApp) {
    section_frame(ui, "Band gains", true, |ui| {
        ui.label(
            RichText::new("Too hot? Raise Threshold · lower Bass")
                .small()
                .color(Color32::GRAY),
        );

        let mut changed = false;
        {
            let t = &mut app.analyzer.tuning;
            changed |= slider(ui, "Input gain", &mut t.input_gain, 0.05..=2.5).changed();
            changed |= slider(ui, "Threshold", &mut t.threshold, 0.0..=0.35).changed();
            changed |= slider(ui, "Sensitivity", &mut t.sensitivity, 0.2..=3.0).changed();
            changed |= slider(ui, "Ceiling", &mut t.ceiling, 0.3..=1.2).changed();
            ui.add_space(4.0);
            ui.label(RichText::new("Per band").small().strong());
            changed |= slider(ui, "Bass", &mut t.bass_gain, 0.05..=2.0).changed();
            changed |= slider(ui, "Low mid", &mut t.low_mid_gain, 0.05..=2.0).changed();
            changed |= slider(ui, "Mid", &mut t.mid_gain, 0.05..=2.0).changed();
            changed |= slider(ui, "High mid", &mut t.high_mid_gain, 0.05..=2.0).changed();
            changed |= slider(ui, "High", &mut t.high_gain, 0.05..=2.5).changed();
            changed |= slider(ui, "Bass tilt", &mut t.spectrum_bass_tilt, 0.1..=1.5)
                .on_hover_text("How much bass owns the start of the ring")
                .changed();
            ui.add_space(4.0);
            changed |= slider(ui, "Attack", &mut t.attack, 0.05..=1.0).changed();
            changed |= slider(ui, "Release", &mut t.release, 0.02..=0.6).changed();
        }

        ui.horizontal(|ui| {
            if ui.button("Reset").clicked() {
                app.analyzer.tuning.reset_defaults();
                changed = true;
            }
            if ui
                .button("Bar venue")
                .on_hover_text("Loads the bundled bar preset")
                .clicked()
            {
                app.apply_preset_by_name("bar");
            }
        });

        if changed {
            app.apply_tuning_from_ui();
        }
    });
}

fn draw_levels_section(ui: &mut egui::Ui, app: &mut LiveVisualizerApp) {
    section_frame(ui, "Live levels", true, |ui| {
        let c = &app.settings.colors;
        level_bar(ui, "RMS", app.features.rms, Color32::from_rgb(180, 180, 200));
        level_bar(ui, "Bass", app.features.bass, rgb_to_color32(c.band_bass));
        level_bar(ui, "Low Mid", app.features.low_mid, rgb_to_color32(c.band_low_mid));
        level_bar(ui, "Mid", app.features.mid, rgb_to_color32(c.band_mid));
        level_bar(ui, "High Mid", app.features.high_mid, rgb_to_color32(c.band_high_mid));
        level_bar(ui, "High", app.features.high, rgb_to_color32(c.band_high));
    });
}

fn draw_visualizer_section(ui: &mut egui::Ui, app: &mut LiveVisualizerApp) {
    section_frame(ui, "Spectrum ring", true, |ui| {
        let mut changed = false;
        {
            let v = &mut app.settings.visualizer;
            ui.label(RichText::new("Style").small().strong());
            ui.horizontal_wrapped(|ui| {
                changed |= ui
                    .selectable_value(&mut v.style, SpectrumStyle::Bars, "Bars")
                    .changed();
                changed |= ui
                    .selectable_value(&mut v.style, SpectrumStyle::Smooth, "Smooth")
                    .changed();
                changed |= ui
                    .selectable_value(&mut v.style, SpectrumStyle::SoftGlow, "Glow")
                    .changed();
            });
            ui.label(RichText::new("Layout").small().strong());
            ui.horizontal_wrapped(|ui| {
                changed |= ui
                    .selectable_value(&mut v.layout, SpectrumLayout::Full, "Full circle")
                    .changed();
                changed |= ui
                    .selectable_value(&mut v.layout, SpectrumLayout::Split, "Split L/R")
                    .on_hover_text(
                        "Mirrored circle: bass/sub as two ear spikes, mids/highs complete the ring",
                    )
                    .changed();
            });
            if v.layout == SpectrumLayout::Split {
                changed |= slider(ui, "Bass ears", &mut v.ear_gain, 0.4..=2.4)
                    .on_hover_text("How far the left/right bass spikes stick out")
                    .changed();
            }

            ui.add_space(6.0);
            ui.label(RichText::new("Ring color").small().strong());
            ui.horizontal_wrapped(|ui| {
                for (mode, label, tip) in [
                    (ColorMode::Rainbow, "Rainbow", "Classic neon spectrum"),
                    (ColorMode::Solid, "Solid", "One color for the whole ring"),
                    (ColorMode::Gradient, "Gradient", "Blend from low → high"),
                    (ColorMode::Bands, "Bands", "Custom color per frequency band"),
                    (ColorMode::Mono, "Mono", "Grayscale"),
                ] {
                    changed |= ui
                        .selectable_value(&mut v.color_mode, mode, label)
                        .on_hover_text(tip)
                        .changed();
                }
            });
        }

        // Legacy Cyan/Amber/Magenta → Solid with that color.
        {
            let mode = app.settings.visualizer.color_mode;
            if let Some(solid) = match mode {
                ColorMode::Cyan => Some(RgbColor::new(0.2, 0.82, 1.0)),
                ColorMode::Amber => Some(RgbColor::new(1.0, 0.67, 0.22)),
                ColorMode::Magenta => Some(RgbColor::new(1.0, 0.22, 0.75)),
                _ => None,
            } {
                app.settings.colors.solid = solid;
                app.settings.visualizer.color_mode = ColorMode::Solid;
                changed = true;
            }
        }

        {
            let mode = app.settings.visualizer.color_mode;
            let colors = &mut app.settings.colors;
            match mode {
                ColorMode::Solid => {
                    changed |= color_picker_row(ui, "Color", &mut colors.solid);
                }
                ColorMode::Gradient => {
                    changed |= color_picker_row(ui, "Low (bass)", &mut colors.gradient_low);
                    changed |= color_picker_row(ui, "High", &mut colors.gradient_high);
                }
                ColorMode::Bands => {
                    ui.label(
                        RichText::new("Band colors (bass → high)")
                            .small()
                            .color(Color32::GRAY),
                    );
                    changed |= color_picker_row(ui, "Bass", &mut colors.band_bass);
                    changed |= color_picker_row(ui, "Low mid", &mut colors.band_low_mid);
                    changed |= color_picker_row(ui, "Mid", &mut colors.band_mid);
                    changed |= color_picker_row(ui, "High mid", &mut colors.band_high_mid);
                    changed |= color_picker_row(ui, "High", &mut colors.band_high);
                }
                _ => {}
            }
            changed |= slider(ui, "Color intensity", &mut colors.rgb_intensity, 0.3..=1.6)
                .on_hover_text("Brightness of spectrum / FFT colors")
                .changed();
        }

        {
            let v = &mut app.settings.visualizer;
            changed |= slider(ui, "Thickness", &mut v.thickness, 0.5..=6.0).changed();
            changed |= slider(ui, "Max length", &mut v.max_bar_length, 0.15..=0.85).changed();
            changed |= slider(ui, "Glow", &mut v.glow, 0.0..=1.5).changed();
            changed |= slider(ui, "Smoothing", &mut v.smoothing, 0.05..=0.9).changed();
        }
        if changed {
            app.mark_settings_dirty();
        }
    });
}

fn draw_particles_section(ui: &mut egui::Ui, app: &mut LiveVisualizerApp) {
    section_frame(ui, "Particles", false, |ui| {
        ui.label(
            RichText::new("Colors follow the spectrum ring mode")
                .small()
                .color(Color32::GRAY),
        );
        let mut changed = false;
        {
            let p = app.particles.settings_mut();
            changed |= ui.checkbox(&mut p.enabled, "Enabled").changed();
            let mut count = p.count as f32;
            if slider(ui, "Count", &mut count, 100.0..=3000.0).changed() {
                p.count = count as u32;
                changed = true;
            }
            changed |= slider(ui, "Size", &mut p.size, 0.5..=6.0).changed();
            changed |= slider(ui, "Speed", &mut p.speed, 0.05..=2.0).changed();
            changed |= slider(ui, "Spread", &mut p.spread, 0.2..=2.0).changed();
            changed |= slider(ui, "Bass reaction", &mut p.bass_reaction, 0.0..=3.0).changed();
            changed |= slider(ui, "Beat reaction", &mut p.beat_reaction, 0.0..=3.0).changed();
            changed |= slider(ui, "Lifetime", &mut p.lifetime, 0.4..=5.0).changed();
            changed |= slider(ui, "Opacity", &mut p.opacity, 0.1..=1.0).changed();
            changed |= slider(ui, "Glow", &mut p.glow, 0.0..=1.5).changed();
            changed |= slider(ui, "Gravity", &mut p.gravity, -1.0..=1.0).changed();
        }
        if changed {
            let s = app.particles.settings().clone();
            app.particles.set_settings(s.clone());
            app.settings.particles = s;
            app.mark_settings_dirty();
        }
    });
}

fn draw_background_section(ui: &mut egui::Ui, app: &mut LiveVisualizerApp) {
    section_frame(ui, "Background", true, |ui| {
        ui.horizontal(|ui| {
            if ui.button("Choose image…").clicked() {
                app.pick_background(ui.ctx());
            }
            if ui.button("Clear").clicked() {
                app.clear_background();
            }
        });
        if let Some(ref path) = app.settings.background_path {
            ui.label(
                RichText::new(truncate_path(path, 42))
                    .small()
                    .color(Color32::GRAY),
            );
        }
        let mut changed = false;
        {
            let s = &mut app.settings.stage;
            changed |= slider(ui, "Brightness", &mut s.background_brightness, 0.1..=1.5).changed();
            changed |= slider(ui, "Darkness", &mut s.background_darkness, 0.0..=0.95).changed();
            changed |= slider(ui, "Zoom", &mut s.background_zoom, 0.8..=1.6).changed();
            changed |= slider(ui, "Pan X", &mut s.background_pan_x, -1.0..=1.0).changed();
            changed |= slider(ui, "Pan Y", &mut s.background_pan_y, -1.0..=1.0).changed();
        }
        if changed {
            app.mark_settings_dirty();
        }
    });
}

fn draw_logo_section(ui: &mut egui::Ui, app: &mut LiveVisualizerApp) {
    section_frame(ui, "Center logo", true, |ui| {
        ui.horizontal(|ui| {
            if ui.button("Add logo…").clicked() {
                app.pick_logo(ui.ctx());
            }
            if ui.button("Add many…").clicked() {
                app.pick_logos(ui.ctx());
            }
            if ui.button("Clear all").clicked() {
                app.clear_logo();
            }
        });

        if !app.settings.logo_paths.is_empty() {
            ui.add_space(4.0);
            let paths = app.settings.logo_paths.clone();
            let active = app.logo_index;
            for (i, path) in paths.iter().enumerate() {
                ui.horizontal(|ui| {
                    let selected = i == active;
                    if ui
                        .selectable_label(selected, truncate_path(path, 34))
                        .clicked()
                    {
                        app.select_logo(i);
                    }
                    if ui.small_button("✕").clicked() {
                        app.remove_logo_at(i);
                    }
                });
            }
            if paths.len() > 1 {
                ui.label(
                    RichText::new(format!(
                        "Cycling {} logos · next in {:.0}s",
                        paths.len(),
                        app.logo_hold_left.max(0.0)
                    ))
                    .small()
                    .color(Color32::GRAY),
                );
            }
        }

        let mut changed = false;
        {
            let s = &mut app.settings.stage;
            ui.add_space(4.0);
            ui.label(RichText::new("Shared").small().strong());
            changed |= slider(ui, "Disc size", &mut s.disc_radius, 0.06..=0.35).changed();
            changed |= color_picker_row(ui, "Disc color", &mut s.disc_color);
            changed |= slider(ui, "Bass bounce", &mut s.bass_pulse, 0.0..=0.35).changed();
            changed |= slider(ui, "Beat bounce", &mut s.beat_pulse, 0.0..=0.45).changed();
            changed |= ui.checkbox(&mut s.disc_ticks, "Disc tick marks").changed();

            ui.add_space(4.0);
            ui.label(RichText::new("Transitions").small().strong());
            changed |= ui
                .checkbox(&mut s.logo_glitch, "Glitch transition")
                .on_hover_text("RGB tear when switching logos (separate from beat glitch)")
                .changed();
            if s.logo_glitch {
                changed |= slider(ui, "Transition amount", &mut s.logo_glitch_amount, 0.2..=1.5)
                    .changed();
            }

            ui.add_space(4.0);
            ui.label(RichText::new("Beat glitch").small().strong());
            changed |= ui
                .checkbox(&mut s.logo_glitch_on_beat, "Glitch on beat")
                .on_hover_text("Short glitch pulse on kicks — independent of logo transitions")
                .changed();
            if s.logo_glitch_on_beat {
                changed |=
                    slider(ui, "Beat amount", &mut s.logo_beat_glitch_amount, 0.2..=1.5).changed();
            }
            if app.logos.len() > 1 {
                changed |= slider(ui, "Default hold", &mut s.logo_hold_secs, 2.0..=30.0)
                    .on_hover_text("Used when a logo's own hold is 0")
                    .changed();
            }
        }

        if !app.logos.is_empty() {
            let idx = app.logo_index.min(app.settings.logo_paths.len().saturating_sub(1));
            app.settings.sync_logo_styles();
            ui.add_space(8.0);
            ui.label(
                RichText::new(format!("This logo (#{})", idx + 1))
                    .small()
                    .strong(),
            );
            {
                let style = app.settings.logo_style_mut(idx);
                changed |= slider(ui, "Size", &mut style.size, 0.25..=1.0).changed();
                changed |= slider(ui, "Offset X", &mut style.offset_x, -0.45..=0.45).changed();
                changed |= slider(ui, "Offset Y", &mut style.offset_y, -0.45..=0.45).changed();
                ui.horizontal(|ui| {
                    let step = 0.02_f32;
                    if ui.button("←").clicked() {
                        style.offset_x = (style.offset_x - step).max(-0.45);
                        changed = true;
                    }
                    if ui.button("→").clicked() {
                        style.offset_x = (style.offset_x + step).min(0.45);
                        changed = true;
                    }
                    if ui.button("↑").clicked() {
                        style.offset_y = (style.offset_y - step).max(-0.45);
                        changed = true;
                    }
                    if ui.button("↓").clicked() {
                        style.offset_y = (style.offset_y + step).min(0.45);
                        changed = true;
                    }
                    if ui.button("Center").clicked() {
                        style.offset_x = 0.0;
                        style.offset_y = 0.0;
                        changed = true;
                    }
                });

                ui.add_space(4.0);
                ui.label(RichText::new("Motion").small().strong());
                ui.horizontal_wrapped(|ui| {
                    for (mode, label) in [
                        (LogoMotion::None, "None"),
                        (LogoMotion::Spin, "Spin"),
                        (LogoMotion::BeatSpin, "Beat spin"),
                        (LogoMotion::Wobble, "Wobble"),
                        (LogoMotion::Pendulum, "Pendulum"),
                    ] {
                        if ui.selectable_value(&mut style.motion, mode, label).changed() {
                            changed = true;
                        }
                    }
                });
                if style.motion != LogoMotion::None {
                    changed |= slider(ui, "Speed", &mut style.spin_speed, 0.02..=1.5).changed();
                    if matches!(style.motion, LogoMotion::Wobble | LogoMotion::Pendulum) {
                        changed |=
                            slider(ui, "Amount", &mut style.motion_amount, 0.05..=1.0).changed();
                    }
                }

                ui.add_space(4.0);
                ui.label(RichText::new("Look").small().strong());
                changed |= color_picker_tint(ui, "Tint", style);
                changed |= slider(ui, "Brightness", &mut style.brightness, 0.2..=1.8).changed();
                changed |= slider(ui, "Opacity", &mut style.opacity, 0.15..=1.0).changed();
                if ui.small_button("Reset tint").clicked() {
                    set_style_tint(style, 1.0, 1.0, 1.0, 1.0);
                    changed = true;
                }

                if app.logos.len() > 1 {
                    ui.add_space(4.0);
                    changed |= slider(ui, "Hold (sec)", &mut style.hold_secs, 0.0..=30.0)
                        .on_hover_text("0 = use default hold")
                        .changed();
                }
                changed |= ui
                    .checkbox(&mut style.glitch_on_beat, "Beat glitch (this logo)")
                    .changed();
            }
        }

        if changed {
            app.mark_settings_dirty();
        }
    });
}

fn set_style_tint(s: &mut crate::config::LogoStyle, r: f32, g: f32, b: f32, bright: f32) {
    s.tint_r = r;
    s.tint_g = g;
    s.tint_b = b;
    s.brightness = bright;
    s.opacity = 1.0;
}

fn logo_style_tint(s: &crate::config::LogoStyle) -> Color32 {
    let b = s.brightness.clamp(0.0, 2.0);
    let a = (s.opacity.clamp(0.0, 1.0) * 255.0) as u8;
    Color32::from_rgba_unmultiplied(
        (s.tint_r.clamp(0.0, 2.0) * b * 255.0).min(255.0) as u8,
        (s.tint_g.clamp(0.0, 2.0) * b * 255.0).min(255.0) as u8,
        (s.tint_b.clamp(0.0, 2.0) * b * 255.0).min(255.0) as u8,
        a,
    )
}

fn draw_display_section(ui: &mut egui::Ui, app: &mut LiveVisualizerApp) {
    section_frame(ui, "Display", true, |ui| {
        ui.checkbox(&mut app.show_debug, "Show FPS overlay");
        if ui
            .checkbox(&mut app.settings_popout, "Pop out this window")
            .changed()
            && app.settings_popout
        {
            app.show_settings = true;
        }
        if ui.checkbox(&mut app.fullscreen, "Fullscreen stage").changed() {
            crate::display::toggle_fullscreen(ui.ctx(), app.fullscreen);
        }
        if ui.button("Toggle fullscreen (F11)").clicked() {
            app.fullscreen = !app.fullscreen;
            crate::display::toggle_fullscreen(ui.ctx(), app.fullscreen);
        }
    });
}

fn slider(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
) -> egui::Response {
    ui.horizontal(|ui| {
        ui.add_sized([110.0, 18.0], egui::Label::new(label));
        ui.add(egui::Slider::new(value, range).show_value(true))
    })
    .inner
}

fn color_picker_row(ui: &mut egui::Ui, label: &str, color: &mut RgbColor) -> bool {
    let mut srgb = color.to_srgb();
    let changed = ui
        .horizontal(|ui| {
            ui.add_sized([110.0, 18.0], egui::Label::new(label));
            ui.color_edit_button_srgb(&mut srgb).changed()
        })
        .inner;
    if changed {
        *color = RgbColor::from_srgb(srgb);
    }
    changed
}

fn color_picker_tint(ui: &mut egui::Ui, label: &str, style: &mut crate::config::LogoStyle) -> bool {
    let mut srgb = [
        (style.tint_r.clamp(0.0, 1.0) * 255.0) as u8,
        (style.tint_g.clamp(0.0, 1.0) * 255.0) as u8,
        (style.tint_b.clamp(0.0, 1.0) * 255.0) as u8,
    ];
    let changed = ui
        .horizontal(|ui| {
            ui.add_sized([110.0, 18.0], egui::Label::new(label));
            ui.color_edit_button_srgb(&mut srgb).changed()
        })
        .inner;
    if changed {
        style.tint_r = srgb[0] as f32 / 255.0;
        style.tint_g = srgb[1] as f32 / 255.0;
        style.tint_b = srgb[2] as f32 / 255.0;
    }
    changed
}

fn rgb_to_color32(c: RgbColor) -> Color32 {
    let s = c.to_srgb();
    Color32::from_rgb(s[0], s[1], s[2])
}

fn level_bar(ui: &mut egui::Ui, label: &str, value: f32, color: Color32) {
    ui.horizontal(|ui| {
        ui.add_sized([72.0, 18.0], egui::Label::new(label));
        let v = value.clamp(0.0, 1.0);
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width() - 48.0, 12.0), Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(rect, 2.0, Color32::from_rgb(28, 28, 36));
        painter.rect_filled(
            egui::Rect::from_min_size(rect.min, egui::vec2(rect.width() * v, rect.height())),
            2.0,
            color,
        );
        ui.label(RichText::new(format!("{v:.2}")).monospace().small());
    });
}

fn truncate_path(path: &str, max: usize) -> String {
    if path.len() <= max {
        path.to_string()
    } else {
        format!("…{}", &path[path.len().saturating_sub(max - 1)..])
    }
}

pub fn draw_stage(ui: &mut egui::Ui, app: &mut LiveVisualizerApp, show_debug: bool) {
    let avail = ui.available_size();
    let (rect, _) = ui.allocate_exact_size(avail, Sense::hover());

    let dt = (app.timing.frame_ms / 1000.0).clamp(1.0 / 240.0, 1.0 / 20.0);
    app.tick_particles(rect.center(), dt);

    let features = &app.features;
    let timing = &app.timing;
    let stage = app.stage().clone();
    let vis = app.settings.visualizer.clone();
    let colors = app.settings.colors.clone();
    let color_mode = vis.color_mode;
    let particles_on = app.particles.settings().enabled;
    let painter = ui.painter_at(rect);

    painter.rect_filled(rect, 0.0, Color32::from_rgb(8, 8, 12));

    if let Some(ref bg) = app.background {
        let img_rect = cover_rect(
            rect,
            bg.size,
            stage.background_zoom,
            stage.background_pan_x,
            stage.background_pan_y,
        );
        let bright = (stage.background_brightness * 255.0).clamp(0.0, 255.0) as u8;
        painter.image(
            bg.texture.id(),
            img_rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::from_rgb(bright, bright, bright),
        );
        let dark_a = (stage.background_darkness * 255.0).clamp(0.0, 255.0) as u8;
        painter.rect_filled(rect, 0.0, Color32::from_black_alpha(dark_a));
    }

    let min_dim = rect.height().min(rect.width());
    let center = rect.center();
    // Whole disc + logo + ring bounce together (Trap Nation style).
    // Kick envelope + beat pulse do the hit; sustained bass is a light swell only.
    let pulse = 1.0
        + features.kick * stage.bass_pulse * 1.15
        + features.beat * stage.beat_pulse
        + features.bass * stage.bass_pulse * 0.22;
    let disc_r = (min_dim * stage.disc_radius * pulse).max(24.0);

    app.particles.draw(&painter, color_mode, &colors);

    draw_spectrum(&painter, center, disc_r, min_dim, features, &vis, &colors);

    // Disc under the logo — customizable fill.
    let disc_rgb = stage.disc_color.to_srgb();
    painter.circle_filled(
        center,
        disc_r,
        Color32::from_rgb(disc_rgb[0], disc_rgb[1], disc_rgb[2]),
    );
    let stroke_rgb = [
        disc_rgb[0].saturating_add(24),
        disc_rgb[1].saturating_add(24),
        disc_rgb[2].saturating_add(30),
    ];
    painter.circle_stroke(
        center,
        disc_r,
        Stroke::new(
            1.5_f32,
            Color32::from_rgb(stroke_rgb[0], stroke_rgb[1], stroke_rgb[2]),
        ),
    );

    let logo_angle = app.logo_draw_angle();
    if stage.disc_ticks {
        draw_disc_ticks(&painter, center, disc_r, logo_angle);
    }

    if !app.logos.is_empty() {
        let style = app.active_logo_style();
        let logo_center = egui::pos2(
            center.x + style.offset_x * disc_r,
            center.y + style.offset_y * disc_r,
        );
        let cur_idx = app.logo_index.min(app.logos.len() - 1);
        let from_idx = app.logo_from_index.min(app.logos.len() - 1);
        let cur = &app.logos[cur_idx];
        let logo_rect = fit_rect(
            egui::Rect::from_center_size(
                logo_center,
                egui::vec2(disc_r * 2.0 * style.size, disc_r * 2.0 * style.size),
            ),
            cur.size,
        );
        let tint = logo_style_tint(&style);
        let switch_g = app.logo_switch_glitch_t * stage.logo_glitch_amount.clamp(0.0, 1.5);
        let beat_g = app.logo_beat_glitch_t * stage.logo_beat_glitch_amount.clamp(0.0, 1.5);
        let cur_tex = cur.texture.id();
        let prev_tex = if from_idx != cur_idx {
            Some(app.logos[from_idx].texture.id())
        } else {
            None
        };
        // Transition glitch first (shows previous logo tearing out).
        // Beat glitch is standalone — only RGB-tears the current logo.
        if switch_g > 0.04 {
            paint_logo_glitch(
                &painter,
                cur_tex,
                prev_tex,
                logo_rect,
                logo_angle,
                tint,
                switch_g,
            );
        } else if beat_g > 0.04 {
            paint_logo_glitch(&painter, cur_tex, None, logo_rect, logo_angle, tint, beat_g);
        } else {
            paint_rotated_image(&painter, cur_tex, logo_rect, logo_angle, tint);
        }
    }

    if app.show_fft_debug {
        draw_fft_graph(&painter, rect, features, color_mode, &colors);
    }

    if show_debug {
        let text = format!(
            "{:.0} FPS  |  {:.1} ms  |  bass {:.2}  |  particles {}",
            timing.fps,
            timing.frame_ms,
            features.bass,
            if particles_on { "on" } else { "off" }
        );
        painter.text(
            rect.left_top() + egui::vec2(12.0, 10.0),
            egui::Align2::LEFT_TOP,
            text,
            egui::FontId::monospace(13.0),
            Color32::from_rgb(0, 220, 180),
        );
    }
}

fn draw_spectrum(
    painter: &egui::Painter,
    center: Pos2,
    disc_r: f32,
    min_dim: f32,
    features: &crate::audio::AudioFeatures,
    vis: &crate::config::VisualizerSettings,
    colors: &ColorSettings,
) {
    let spectrum = &features.spectrum;
    if spectrum.is_empty() {
        return;
    }

    let n = spectrum.len();
    let mut smoothed = spectrum.clone();
    let smooth_amt = (vis.smoothing * 0.32).clamp(0.0, 0.4);
    if smooth_amt > 0.02 {
        let prev = smoothed.clone();
        for i in 0..n {
            let t = i as f32 / n as f32;
            let amt = if t < 0.18 { smooth_amt * 0.22 } else { smooth_amt };
            let a = if i == 0 { prev[i] } else { prev[i - 1] };
            let b = prev[i];
            let c = if i + 1 >= n { prev[i] } else { prev[i + 1] };
            let blended = b * (1.0 - amt) + (a + c) * 0.5 * amt;
            smoothed[i] = blended.max(b * 0.92);
        }
    }

    let base_r = disc_r + 4.0;
    let mut max_len = (disc_r * vis.max_bar_length.clamp(0.15, 1.0))
        .min(min_dim * 0.16)
        .min(disc_r * 1.05)
        .max(8.0);
    if vis.layout == SpectrumLayout::Split {
        max_len *= 1.28;
    }
    let thickness = vis.thickness.clamp(0.8, 4.0);
    let intensity = 0.9 * app_color_intensity(vis);
    let glow = vis.glow * 0.4;
    let pts = collect_ring_points(&smoothed, vis, features);

    match vis.style {
        SpectrumStyle::Bars => {
            draw_ring_bars(
                painter,
                center,
                base_r,
                max_len,
                thickness,
                glow,
                intensity,
                vis.color_mode,
                colors,
                &pts,
            );
        }
        SpectrumStyle::Smooth | SpectrumStyle::SoftGlow => {
            draw_ring_smooth(
                painter,
                center,
                base_r,
                max_len,
                glow,
                intensity,
                vis.color_mode,
                colors,
                matches!(vis.style, SpectrumStyle::SoftGlow),
                vis.layout == SpectrumLayout::Full,
                &pts,
            );
        }
    }
}

#[derive(Clone, Copy)]
struct SpecPoint {
    t: f32,
    mag: f32,
    hue: f32,
}

fn collect_ring_points(
    smoothed: &[f32],
    vis: &crate::config::VisualizerSettings,
    features: &crate::audio::AudioFeatures,
) -> Vec<SpecPoint> {
    let n = smoothed.len().max(8);
    match vis.layout {
        SpectrumLayout::Full => (0..n)
            .map(|i| {
                let t = (i as f32 + 0.5) / n as f32;
                SpecPoint {
                    t,
                    mag: smoothed[i].clamp(0.0, 1.2),
                    hue: t,
                }
            })
            .collect(),
        SpectrumLayout::Split => {
            // Full mirrored circle: lows at the bottom, highs toward the top,
            // bass/sub as ear lobes at ~2 o'clock and 10 o'clock.
            let cut = ((n as f32) * 0.20) as usize;
            let rest = (n - cut).max(8);
            let ear_gain = vis.ear_gain.clamp(0.2, 2.8);
            let punch = (features.bass * 0.55 + features.kick * 0.95).min(1.25);
            (0..n)
                .map(|i| {
                    let t = (i as f32 + 0.5) / n as f32;
                    let u = if t <= 0.5 { t * 2.0 } else { (1.0 - t) * 2.0 };
                    SpecPoint {
                        t,
                        mag: trap_nation_mag(smoothed, cut, rest, u, punch, ear_gain),
                        hue: (0.06 + u * 0.82).clamp(0.0, 1.0),
                    }
                })
                .collect()
        }
    }
}

fn trap_nation_mag(
    smoothed: &[f32],
    cut: usize,
    rest: usize,
    u: f32,
    punch: f32,
    ear_gain: f32,
) -> f32 {
    // Ear peak ~36° off 12 o'clock → 2 o'clock / 10 o'clock.
    let ear_c = 0.20_f32;
    let ear_w = 0.075_f32;
    let x = (u - ear_c) / ear_w;
    let ear = (-x * x * 2.1).exp();

    let bass_u = ((u - (ear_c - ear_w * 2.2)) / (ear_w * 4.4)).clamp(0.0, 1.0);
    let bass_tex = interp_spec(smoothed, bass_u * (cut.saturating_sub(1) as f32));
    let ear_mag = (bass_tex * 0.62 + punch * 0.72) * ear * ear_gain;

    let body = interp_spec(smoothed, cut as f32 + u * (rest.saturating_sub(1) as f32));
    let body = body * (1.0 - ear * 0.62);

    (ear_mag + body).clamp(0.0, 1.45)
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
    a * (1.0 - f) + b * f
}

fn ring_angle(t: f32) -> f32 {
    t * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2
}

fn ring_pos(center: Pos2, t: f32, radius: f32) -> Pos2 {
    let (s, c) = ring_angle(t).sin_cos();
    Pos2::new(center.x + c * radius, center.y + s * radius)
}

fn draw_ring_bars(
    painter: &egui::Painter,
    center: Pos2,
    base_r: f32,
    max_len: f32,
    thickness: f32,
    glow: f32,
    intensity: f32,
    mode: ColorMode,
    colors: &ColorSettings,
    pts: &[SpecPoint],
) {
    for p in pts {
        let len = p.mag.clamp(0.0, 1.45) * max_len;
        let inner = ring_pos(center, p.t, base_r);
        let outer = ring_pos(center, p.t, base_r + len);
        let color = spectrum_color(p.hue, intensity, mode, colors);
        if glow > 0.05 {
            painter.line_segment(
                [inner, outer],
                Stroke::new(thickness * 1.5, with_alpha(color, (36.0 * glow) as u8)),
            );
        }
        painter.line_segment([inner, outer], Stroke::new(thickness, color));
    }
}

fn draw_ring_smooth(
    painter: &egui::Painter,
    center: Pos2,
    base_r: f32,
    max_len: f32,
    glow: f32,
    intensity: f32,
    mode: ColorMode,
    colors: &ColorSettings,
    soft_glow: bool,
    rainbow_wrap: bool,
    pts: &[SpecPoint],
) {
    if pts.len() < 2 {
        return;
    }
    for i in 0..pts.len() {
        let a = pts[i];
        let b = pts[(i + 1) % pts.len()];
        let len0 = a.mag.clamp(0.0, 1.45) * max_len;
        let len1 = b.mag.clamp(0.0, 1.45) * max_len;
        let inner0 = ring_pos(center, a.t, base_r);
        let inner1 = ring_pos(center, b.t, base_r);
        let outer0 = ring_pos(center, a.t, base_r + len0);
        let outer1 = ring_pos(center, b.t, base_r + len1);
        let hue = if rainbow_wrap && i + 1 == pts.len() {
            1.0
        } else {
            (a.hue + b.hue) * 0.5
        };
        let color = spectrum_color(hue, intensity, mode, colors);

        if soft_glow && glow > 0.05 {
            let gc = with_alpha(color, (32.0 * glow) as u8);
            painter.add(Shape::convex_polygon(
                vec![
                    inner0,
                    ring_pos(center, a.t, base_r + len0 * 1.04),
                    ring_pos(center, b.t, base_r + len1 * 1.04),
                    inner1,
                ],
                gc,
                Stroke::NONE,
            ));
        }
        painter.add(Shape::convex_polygon(
            vec![inner0, outer0, outer1, inner1],
            with_alpha(color, 215),
            Stroke::NONE,
        ));
        painter.line_segment(
            [outer0, outer1],
            Stroke::new(1.0_f32, with_alpha(color, 230)),
        );
    }
}

fn paint_logo_glitch(
    painter: &egui::Painter,
    current: egui::TextureId,
    previous: Option<egui::TextureId>,
    rect: egui::Rect,
    angle: f32,
    tint: Color32,
    amount: f32,
) {
    let a = amount.clamp(0.0, 1.5);
    let shake = a * 14.0;
    let seed = (a * 97.0 + rect.center().x * 0.01) as i32;

    if let Some(prev) = previous {
        let fade = (a * 180.0).min(160.0) as u8;
        let ghost = Color32::from_rgba_unmultiplied(tint.r(), tint.g(), tint.b(), fade);
        let jog = ((seed % 7) as f32 - 3.0) * shake * 0.35;
        paint_rotated_image(
            painter,
            prev,
            rect.translate(egui::vec2(jog, -jog * 0.4)),
            angle,
            ghost,
        );
    }

    let ox = shake * 0.55;
    let r_tint = Color32::from_rgba_unmultiplied(tint.r(), 0, 0, ((tint.a() as f32) * 0.55) as u8);
    let g_tint = Color32::from_rgba_unmultiplied(0, tint.g(), 0, ((tint.a() as f32) * 0.55) as u8);
    let b_tint = Color32::from_rgba_unmultiplied(0, 0, tint.b(), ((tint.a() as f32) * 0.55) as u8);
    paint_rotated_image(
        painter,
        current,
        rect.translate(egui::vec2(-ox, 0.0)),
        angle,
        r_tint,
    );
    paint_rotated_image(
        painter,
        current,
        rect.translate(egui::vec2(0.0, ox * 0.25)),
        angle,
        g_tint,
    );
    paint_rotated_image(
        painter,
        current,
        rect.translate(egui::vec2(ox, 0.0)),
        angle,
        b_tint,
    );

    let slices = 6;
    for i in 0..slices {
        let t0 = i as f32 / slices as f32;
        let t1 = (i + 1) as f32 / slices as f32;
        let band = egui::Rect::from_min_max(
            egui::pos2(rect.left(), rect.top() + rect.height() * t0),
            egui::pos2(rect.right(), rect.top() + rect.height() * t1),
        );
        let uv = egui::Rect::from_min_max(egui::pos2(0.0, t0), egui::pos2(1.0, t1));
        let jog = (((seed + i * 13) % 11) as f32 - 5.0) * shake * 0.22;
        let slice_rect = band.translate(egui::vec2(jog, 0.0));
        let alpha =
            ((tint.a() as f32) * (0.55 + 0.45 * (1.0 - a.min(1.0)))).clamp(40.0, 255.0) as u8;
        let st = Color32::from_rgba_unmultiplied(tint.r(), tint.g(), tint.b(), alpha);
        paint_image_uv(
            painter,
            current,
            slice_rect,
            uv,
            angle,
            rect.center(),
            st,
        );
    }

    let settle = ((1.0 - a.min(1.0)) * tint.a() as f32).clamp(0.0, 255.0) as u8;
    if settle > 20 {
        paint_rotated_image(
            painter,
            current,
            rect,
            angle,
            Color32::from_rgba_unmultiplied(tint.r(), tint.g(), tint.b(), settle),
        );
    }
}

fn paint_image_uv(
    painter: &egui::Painter,
    texture: egui::TextureId,
    rect: egui::Rect,
    uv: egui::Rect,
    angle: f32,
    pivot: Pos2,
    tint: Color32,
) {
    let mut mesh = Mesh::with_texture(texture);
    mesh.add_rect_with_uv(rect, uv, tint);
    if angle.abs() > 0.0001 {
        mesh.rotate(egui::emath::Rot2::from_angle(angle), pivot);
    }
    painter.add(Shape::mesh(mesh));
}

fn paint_rotated_image(
    painter: &egui::Painter,
    texture: egui::TextureId,
    rect: egui::Rect,
    angle: f32,
    tint: Color32,
) {
    if angle.abs() < 0.0001 {
        painter.image(
            texture,
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            tint,
        );
        return;
    }
    let mut mesh = Mesh::with_texture(texture);
    let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    mesh.add_rect_with_uv(rect, uv, tint);
    mesh.rotate(egui::emath::Rot2::from_angle(angle), rect.center());
    painter.add(Shape::mesh(mesh));
}

fn draw_disc_ticks(painter: &egui::Painter, center: Pos2, disc_r: f32, angle: f32) {
    let n = 12;
    let color = Color32::from_rgba_unmultiplied(70, 70, 85, 160);
    for i in 0..n {
        let a = angle + (i as f32) * std::f32::consts::TAU / n as f32;
        let (s, c) = a.sin_cos();
        let inner = disc_r * 0.88;
        let outer = disc_r * 0.97;
        painter.line_segment(
            [
                egui::pos2(center.x + c * inner, center.y + s * inner),
                egui::pos2(center.x + c * outer, center.y + s * outer),
            ],
            Stroke::new(1.5_f32, color),
        );
    }
}

fn app_color_intensity(vis: &crate::config::VisualizerSettings) -> f32 {
    match vis.color_mode {
        ColorMode::Mono => 0.85,
        _ => 1.0,
    }
}

fn draw_fft_graph(
    painter: &egui::Painter,
    rect: egui::Rect,
    features: &crate::audio::AudioFeatures,
    mode: ColorMode,
    colors: &ColorSettings,
) {
    let graph_h = 110.0_f32.min(rect.height() * 0.2);
    let graph = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 16.0, rect.bottom() - graph_h - 16.0),
        egui::pos2(rect.right() - 16.0, rect.bottom() - 16.0),
    );
    painter.rect_filled(graph, 4.0, Color32::from_rgba_unmultiplied(20, 20, 28, 200));
    let n = features.spectrum.len().max(1);
    let bar_w = graph.width() / n as f32;
    for (i, &mag) in features.spectrum.iter().enumerate() {
        let h = mag.clamp(0.0, 1.0) * (graph.height() - 12.0);
        let x = graph.left() + i as f32 * bar_w;
        let bar = egui::Rect::from_min_max(
            egui::pos2(x + 0.5, graph.bottom() - h),
            egui::pos2(x + bar_w - 0.5, graph.bottom()),
        );
        painter.rect_filled(bar, 0.0, spectrum_color(i as f32 / n as f32, 0.9, mode, colors));
    }
}

fn spectrum_color(t: f32, intensity: f32, mode: ColorMode, colors: &ColorSettings) -> Color32 {
    let rgb = colors.sample(mode, t, intensity);
    Color32::from_rgb(
        (rgb.r * 255.0) as u8,
        (rgb.g * 255.0) as u8,
        (rgb.b * 255.0) as u8,
    )
}

fn with_alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}
