//! egui settings panel and stage preview.

use super::media::{cover_rect, fit_rect};
use super::state::LiveVisualizerApp;
use crate::config::{
    BarCap, BeatSettings, ColorMode, ColorSettings, LogoMotion, RgbColor, ScopeColorMode,
    SpectrumLayout, SpectrumStyle, CENTER_SCOPE_ID, CENTER_SPECTRUM_ID,
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
            if ui
                .button("Reset to default")
                .on_hover_text("Restore the built-in factory look and tuning (cannot be overwritten)")
                .clicked()
            {
                app.reset_to_factory();
            }
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
                "Session autosaves · user presets → {}",
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
                    let locked = crate::config::is_locked_preset(name);
                    let selected = !app.preset_dirty
                        && app.settings.last_preset.as_deref() == Some(name.as_str());
                    let label = if pending.as_deref() == Some(name.as_str()) {
                        format!("{name}  (click again)")
                    } else if locked {
                        format!("{name}  🔒")
                    } else {
                        name.clone()
                    };
                    let response = ui.selectable_label(selected, label);
                    let response = if locked {
                        response.on_hover_text("Factory preset — always resets to recommended settings")
                    } else {
                        response
                    };
                    if response.clicked() {
                        if locked {
                            app.reset_to_factory();
                        } else {
                            app.apply_preset_by_name(name);
                        }
                    }
                    ui.end_row();
                }
            });

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Save as");
            ui.text_edit_singleline(&mut app.preset_save_name);
            let locked_name = crate::config::is_locked_preset(&app.preset_save_name);
            let save_label = if locked_name {
                "Locked"
            } else if app.preset_names.iter().any(|n| n == &app.preset_save_name) {
                "Overwrite"
            } else {
                "Save"
            };
            let save = ui
                .add_enabled(
                    !locked_name,
                    egui::Button::new(save_label),
                )
                .on_hover_text(if locked_name {
                    "Choose a different name — default cannot be overwritten"
                } else {
                    "Writes a named preset file (session already autosaves)"
                });
            if save.clicked() {
                app.save_current_preset();
            }
        });
        ui.label(
            RichText::new("Letters, numbers, - and _ only (spaces become _). default is factory-locked.")
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

        ui.add_space(6.0);
        ui.label(RichText::new("Disc bounce").small().strong());
        {
            let s = &mut app.settings.stage;
            changed |= slider(ui, "Bass bounce", &mut s.bass_pulse, 0.0..=0.35)
                .on_hover_text("Disc swell from kick / bass energy")
                .changed();
            changed |= slider(ui, "Beat bounce", &mut s.beat_pulse, 0.0..=0.45)
                .on_hover_text("Disc punch on detected beats")
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
            if ui
                .button("Reset tuning")
                .on_hover_text("Restore factory band gains")
                .clicked()
            {
                app.analyzer.tuning.reset_defaults();
                changed = true;
            }
            if ui
                .button("Reset all to default")
                .on_hover_text("Factory look, colors, beat, particles, and tuning")
                .clicked()
            {
                app.reset_to_factory();
                changed = false;
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
        level_bar(ui, "Sub", app.features.sub, Color32::from_rgb(120, 80, 220));
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
                    .selectable_value(&mut v.layout, SpectrumLayout::Mirrored, "Mirrored")
                    .on_hover_text(
                        "Recommended: bass at the bottom, same spectrum on both sides",
                    )
                    .changed();
                changed |= ui
                    .selectable_value(&mut v.layout, SpectrumLayout::Continuous, "Continuous")
                    .on_hover_text("Frequency increases clockwise around the full ring")
                    .changed();
                changed |= ui
                    .selectable_value(&mut v.layout, SpectrumLayout::Split, "Split ears")
                    .on_hover_text(
                        "Legacy Trap Nation layout: bass/sub as ear spikes, mids/highs complete the ring",
                    )
                    .changed();
            });
            if v.layout == SpectrumLayout::Split {
                ui.label(RichText::new("Bass ears").small().strong());
                ui.horizontal(|ui| {
                    changed |= ui
                        .selectable_value(&mut v.ear_count, 2, "2")
                        .on_hover_text("One ear per side (~2 / 10 o'clock)")
                        .changed();
                    changed |= ui
                        .selectable_value(&mut v.ear_count, 4, "4")
                        .on_hover_text("Two ears per side")
                        .changed();
                });
                changed |= slider(ui, "Ear height", &mut v.ear_gain, 0.4..=2.4)
                    .on_hover_text("How far the bass spikes stick out")
                    .changed();
                changed |= slider(ui, "Ear angle", &mut v.ear_angle, 0.08..=0.42)
                    .on_hover_text("Where ears sit along the half-ring (lower = closer to top)")
                    .changed();
                changed |= slider(ui, "Ear width", &mut v.ear_width, 0.04..=0.14)
                    .on_hover_text("How wide each lobe is")
                    .changed();
                changed |= slider(ui, "Ear lift", &mut v.ear_radius, 0.0..=0.28)
                    .on_hover_text("Push ear bases outward from the disc")
                    .changed();
            }

            ui.add_space(6.0);
            ui.label(RichText::new("Ring color").small().strong());
            ui.horizontal_wrapped(|ui| {
                for (mode, label, tip) in [
                    (ColorMode::Rainbow, "Rainbow", "Cyan → blue → violet → magenta along frequency"),
                    (ColorMode::Solid, "Solid", "One color for the whole ring"),
                    (ColorMode::Gradient, "Gradient", "Blend from low → high"),
                    (ColorMode::Cycle, "Cycle", "Scrolling rainbow"),
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
            match mode {
                ColorMode::Solid => {
                    changed |=
                        color_picker_row(ui, "Color", &mut app.settings.colors.solid);
                }
                ColorMode::Gradient => {
                    changed |= color_picker_row(
                        ui,
                        "Low (bass)",
                        &mut app.settings.colors.gradient_low,
                    );
                    changed |=
                        color_picker_row(ui, "High", &mut app.settings.colors.gradient_high);
                }
                ColorMode::Cycle => {
                    changed |= slider(
                        ui,
                        "Cycle speed",
                        &mut app.settings.visualizer.cycle_speed,
                        0.02..=1.5,
                    )
                    .changed();
                }
                ColorMode::Bands => {
                    ui.label(
                        RichText::new("Band colors (bass → high)")
                            .small()
                            .color(Color32::GRAY),
                    );
                    changed |=
                        color_picker_row(ui, "Bass", &mut app.settings.colors.band_bass);
                    changed |= color_picker_row(
                        ui,
                        "Low mid",
                        &mut app.settings.colors.band_low_mid,
                    );
                    changed |= color_picker_row(ui, "Mid", &mut app.settings.colors.band_mid);
                    changed |= color_picker_row(
                        ui,
                        "High mid",
                        &mut app.settings.colors.band_high_mid,
                    );
                    changed |=
                        color_picker_row(ui, "High", &mut app.settings.colors.band_high);
                }
                _ => {}
            }
            changed |= slider(
                ui,
                "Color intensity",
                &mut app.settings.colors.rgb_intensity,
                0.3..=1.6,
            )
            .on_hover_text("Brightness of spectrum / FFT colors")
            .changed();
        }

        {
            let v = &mut app.settings.visualizer;
            let mut count = v.segment_count as f32;
            if slider(ui, "Bar count", &mut count, 64.0..=384.0)
                .on_hover_text("Dense rectangular bars around the ring")
                .changed()
            {
                v.segment_count = count.round() as u32;
                changed = true;
            }
            changed |= slider(ui, "Bar fill", &mut v.bar_fill, 0.5..=0.9)
                .on_hover_text("How much of each slot the bar occupies (rest is the gap)")
                .changed();
            changed |= slider(ui, "Rotation", &mut v.rotation_offset, 0.0..=1.0)
                .on_hover_text("0.5 puts bass at the bottom in Mirrored mode")
                .changed();
            changed |= ui
                .checkbox(&mut v.reverse_spectrum, "Reverse frequency direction")
                .changed();
            ui.horizontal(|ui| {
                ui.add_sized([110.0, 18.0], egui::Label::new("Bar ends"));
                changed |= ui
                    .selectable_value(&mut v.bar_cap, BarCap::Square, "Square")
                    .changed();
                changed |= ui
                    .selectable_value(&mut v.bar_cap, BarCap::Rounded, "Rounded")
                    .changed();
            });
            changed |= slider(ui, "Thickness", &mut v.thickness, 0.5..=6.0)
                .on_hover_text("Used by Smooth / Glow styles")
                .changed();
            changed |= slider(ui, "Min height", &mut v.min_bar_height, 0.004..=0.03)
                .on_hover_text("Resting crown height (viewport fraction)")
                .changed();
            changed |= slider(ui, "Max length", &mut v.max_bar_length, 0.15..=0.85).changed();
            changed |= slider(ui, "Glow", &mut v.glow, 0.0..=1.5).changed();
            changed |= slider(ui, "Smoothing", &mut v.smoothing, 0.05..=0.95)
                .on_hover_text("Spatial blur on the Smooth / Glow ring body")
                .changed();
            changed |= slider(ui, "Outline width", &mut v.outline_width, 0.5..=3.5)
                .on_hover_text("Outer rim stroke (Smooth / Glow)")
                .changed();
            changed |= slider(ui, "Outline smooth", &mut v.outline_smooth, 0.0..=1.0)
                .on_hover_text("Extra blur on the outer outline for a cleaner edge")
                .changed();
            ui.add_space(4.0);
            ui.label(RichText::new("Analysis").small().strong());
            changed |= slider(ui, "Attack ms", &mut v.attack_ms, 12.0..=80.0)
                .on_hover_text("How fast bars rise into transients")
                .changed();
            changed |= slider(ui, "Release ms", &mut v.release_ms, 80.0..=400.0)
                .on_hover_text("How slowly bars fall after a hit")
                .changed();
            changed |= slider(ui, "Noise floor", &mut v.noise_floor_db, -90.0..=-40.0)
                .on_hover_text("dB below this does not move the ring")
                .changed();
            changed |= slider(ui, "Ceiling dB", &mut v.ceiling_db, -30.0..=0.0)
                .on_hover_text("dB that maps to full bar height")
                .changed();
            changed |= slider(ui, "Response", &mut v.response_exponent, 1.0..=2.2)
                .on_hover_text("Higher = only strong frequencies get tall")
                .changed();
            changed |= slider(ui, "Freq smooth", &mut v.frequency_smoothing, 0.0..=0.8)
                .on_hover_text("Blur neighboring bands so bars don't spike alone")
                .changed();
            changed |= slider(ui, "Transient", &mut v.transient_scale, 0.0..=0.5)
                .on_hover_text("Extra punch on kicks/snares")
                .changed();
            changed |= slider(ui, "Bass weight", &mut v.bass_response, 0.3..=2.0).changed();
            changed |= slider(ui, "Mid weight", &mut v.mid_response, 0.3..=2.0).changed();
            changed |= slider(ui, "Treble weight", &mut v.high_response, 0.3..=2.0).changed();
            changed |= slider(ui, "Min freq", &mut v.min_frequency, 20.0..=80.0)
                .on_hover_text("Analysis floor. Bar mode still skips sub rumble (~20–65 Hz) on the ring")
                .changed();
            changed |= slider(ui, "Max freq", &mut v.max_frequency, 8_000.0..=20_000.0).changed();
            changed |= ui.checkbox(&mut v.show_base_ring, "Show dim base ring").changed();
            if v.show_base_ring {
                changed |= slider(ui, "Ring width", &mut v.base_ring_width, 0.4..=3.0).changed();
                changed |= slider(ui, "Ring bright", &mut v.base_ring_brightness, 0.04..=0.5)
                    .changed();
            }
        }
        if changed {
            app.mark_settings_dirty();
        }
    });
}

fn draw_particles_section(ui: &mut egui::Ui, app: &mut LiveVisualizerApp) {
    section_frame(ui, "Particles", false, |ui| {
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

            ui.add_space(6.0);
            ui.label(RichText::new("Particle color").small().strong());
            ui.horizontal_wrapped(|ui| {
                for (mode, label) in [
                    (ColorMode::Rainbow, "Rainbow"),
                    (ColorMode::Solid, "Solid"),
                    (ColorMode::Gradient, "Gradient"),
                    (ColorMode::Cycle, "Cycle"),
                ] {
                    changed |= ui.selectable_value(&mut p.color_mode, mode, label).changed();
                }
            });
            match p.color_mode {
                ColorMode::Solid => {
                    changed |= color_picker_row(ui, "Color", &mut p.solid);
                }
                ColorMode::Gradient => {
                    changed |= color_picker_row(ui, "Color A", &mut p.gradient_low);
                    changed |= color_picker_row(ui, "Color B", &mut p.gradient_high);
                }
                ColorMode::Cycle => {
                    changed |= slider(ui, "Cycle speed", &mut p.cycle_speed, 0.02..=1.5).changed();
                }
                _ => {}
            }
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
        ui.horizontal_wrapped(|ui| {
            if ui.button("Add logo…").clicked() {
                app.pick_logo(ui.ctx());
            }
            if ui.button("Add many…").clicked() {
                app.pick_logos(ui.ctx());
            }
            if ui
                .button("Add scope")
                .on_hover_text("Oscilloscope as a playlist item you can cycle into")
                .clicked()
            {
                app.push_center_effect(CENTER_SCOPE_ID);
            }
            if ui
                .button("Add spectrum")
                .on_hover_text("Mirrored condensed FFT as a playlist item")
                .clicked()
            {
                app.push_center_effect(CENTER_SPECTRUM_ID);
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
                    let label = if crate::config::is_center_effect(path) {
                        crate::config::center_item_label(path)
                    } else {
                        truncate_path(path, 34)
                    };
                    if ui.selectable_label(selected, label).clicked() {
                        app.select_logo(i);
                    }
                    if ui.small_button("✕").clicked() {
                        app.remove_logo_at(i);
                    }
                });
            }
            if paths.len() > 1 {
                let status = if app.settings.stage.logo_autoplay {
                    format!(
                        "Autoplay · {} items · next in {:.0}s",
                        paths.len(),
                        app.logo_hold_left.max(0.0)
                    )
                } else {
                    format!("{} items · pinned (enable Autoplay to rotate)", paths.len())
                };
                ui.label(RichText::new(status).small().color(Color32::GRAY));
            }
        }

        let playlist_len = app.playlist_len();
        let (mut changed, resume_hold) = {
            let mut changed = false;
            let s = &mut app.settings.stage;
            ui.add_space(4.0);
            ui.label(RichText::new("Oscilloscope look").small().strong());
            changed |= slider(ui, "Scope gain", &mut s.scope_gain, 0.2..=3.0).changed();
            changed |=
                slider(ui, "Scope thickness", &mut s.scope_thickness, 0.8..=5.0).changed();
            ui.horizontal_wrapped(|ui| {
                for (mode, label) in [
                    (ScopeColorMode::Solid, "Solid"),
                    (ScopeColorMode::Rainbow, "Rainbow"),
                    (ScopeColorMode::Gradient, "Gradient"),
                    (ScopeColorMode::Cycle, "Cycle"),
                ] {
                    changed |= ui
                        .selectable_value(&mut s.scope_color_mode, mode, label)
                        .changed();
                }
            });
            match s.scope_color_mode {
                ScopeColorMode::Solid => {
                    changed |= color_picker_row(ui, "Scope color", &mut s.scope_color);
                }
                ScopeColorMode::Gradient => {
                    changed |= color_picker_row(ui, "Scope A", &mut s.scope_color);
                    changed |= color_picker_row(ui, "Scope B", &mut s.scope_color_b);
                }
                ScopeColorMode::Cycle => {
                    changed |=
                        slider(ui, "Scope cycle", &mut s.scope_cycle_speed, 0.02..=1.5).changed();
                }
                ScopeColorMode::Rainbow => {}
            }

            ui.add_space(6.0);
            ui.label(RichText::new("Spectrum look").small().strong());
            changed |= slider(ui, "Spec gain", &mut s.spectrum_gain, 0.2..=3.0).changed();
            changed |=
                slider(ui, "Spec thickness", &mut s.spectrum_thickness, 0.8..=5.0).changed();
            ui.horizontal_wrapped(|ui| {
                for (mode, label) in [
                    (ScopeColorMode::Solid, "Solid"),
                    (ScopeColorMode::Rainbow, "Rainbow"),
                    (ScopeColorMode::Gradient, "Gradient"),
                    (ScopeColorMode::Cycle, "Cycle"),
                ] {
                    changed |= ui
                        .selectable_value(&mut s.spectrum_color_mode, mode, label)
                        .changed();
                }
            });
            match s.spectrum_color_mode {
                ScopeColorMode::Solid => {
                    changed |= color_picker_row(ui, "Spec color", &mut s.spectrum_color);
                }
                ScopeColorMode::Gradient => {
                    changed |= color_picker_row(ui, "Spec A", &mut s.spectrum_color);
                    changed |= color_picker_row(ui, "Spec B", &mut s.spectrum_color_b);
                }
                ScopeColorMode::Cycle => {
                    changed |= slider(ui, "Spec cycle", &mut s.spectrum_cycle_speed, 0.02..=1.5)
                        .changed();
                }
                ScopeColorMode::Rainbow => {}
            }

            ui.add_space(4.0);
            ui.label(RichText::new("Shared").small().strong());
            changed |= slider(ui, "Disc size", &mut s.disc_radius, 0.06..=0.35).changed();
            changed |= color_picker_row(ui, "Disc color", &mut s.disc_color);
            changed |= ui.checkbox(&mut s.disc_ticks, "Disc tick marks").changed();

            ui.add_space(4.0);
            ui.label(RichText::new("Playlist").small().strong());
            let was_auto = s.logo_autoplay;
            changed |= ui
                .checkbox(&mut s.logo_autoplay, "Autoplay")
                .on_hover_text("Rotate through playlist items. Selecting one turns this off.")
                .changed();
            let resume_autoplay = s.logo_autoplay && !was_auto;
            if playlist_len > 1 {
                changed |= slider(ui, "Default hold", &mut s.logo_hold_secs, 2.0..=30.0)
                    .on_hover_text("Seconds each item stays when Autoplay is on")
                    .changed();
            }
            let resume_hold = if resume_autoplay {
                Some(s.logo_hold_secs.max(1.0))
            } else {
                None
            };

            ui.add_space(4.0);
            ui.label(RichText::new("Transitions").small().strong());
            changed |= ui
                .checkbox(&mut s.crt_transition, "CRT effect wipe")
                .on_hover_text(
                    "TV turn-off style when switching to/from Scope or Spectrum (flatten → line → center)",
                )
                .changed();
            changed |= ui
                .checkbox(&mut s.logo_glitch, "Glitch transition")
                .on_hover_text("RGB tear for image switches (when CRT wipe is not used)")
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
            (changed, resume_hold)
        };
        if let Some(hold) = resume_hold {
            app.logo_hold_left = hold;
        }

        if playlist_len > 0 {
            let idx = app.logo_index.min(app.settings.logo_paths.len().saturating_sub(1));
            app.settings.sync_logo_styles();
            let active_path = app.settings.logo_paths.get(idx).cloned().unwrap_or_default();
            let is_image = !crate::config::is_center_effect(&active_path);
            ui.add_space(8.0);
            ui.label(
                RichText::new(format!(
                    "This item (#{}) — {}",
                    idx + 1,
                    crate::config::center_item_label(&active_path)
                ))
                .small()
                .strong(),
            );
            {
                let style = app.settings.logo_style_mut(idx);
                if is_image {
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
                }

                if playlist_len > 1 {
                    ui.add_space(4.0);
                    changed |= slider(ui, "Hold (sec)", &mut style.hold_secs, 0.0..=30.0)
                        .on_hover_text("0 = use default hold")
                        .changed();
                }
                changed |= ui
                    .checkbox(&mut style.glitch_on_beat, "Beat glitch (this item)")
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

    let features = app.features.clone();
    let timing = app.timing.clone();
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
    let time = ui.input(|i| i.time as f32);
    // Whole disc + logo + ring bounce together (Trap Nation style).
    // Soft floors + curve so quiet tracks don't jitter the outline.
    let kick_s = features.kick.powf(1.12);
    let beat_s = features.beat.powf(1.05);
    let bass_s = (features.bass - 0.1).max(0.0).powf(1.1);
    let pulse = (1.0
        + kick_s * stage.bass_pulse * 1.05
        + beat_s * stage.beat_pulse * 0.92
        + bass_s * stage.bass_pulse * 0.14)
        .clamp(1.0, 1.42);
    let disc_r = (min_dim * stage.disc_radius * pulse).max(24.0);

    app.particles.draw(&painter);

    draw_spectrum(
        &mut app.radial_spectrum,
        &painter,
        center,
        disc_r,
        min_dim,
        &features,
        &vis,
        &colors,
        time,
    );

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

    let dt = (app.timing.frame_ms / 1000.0).clamp(1.0 / 240.0, 1.0 / 20.0);
    if app.playlist_len() > 0 {
        use crate::app::state::CrtPhase;
        let (sx, sy, alpha) = app.crt_phase.factors();
        let draw_idx = match app.crt_phase {
            CrtPhase::Out(_) => app.logo_from_index.min(app.playlist_len() - 1),
            _ => app.logo_index.min(app.playlist_len() - 1),
        };
        draw_playlist_item(
            &painter,
            app,
            draw_idx,
            center,
            disc_r,
            logo_angle,
            &features,
            &stage,
            time,
            dt,
            sx,
            sy,
            alpha,
        );
        // Bright CRT scanline while flattened.
        if app.crt_phase.is_busy() && sy < 0.12 {
            let line_w = disc_r * 2.0 * sx.clamp(0.04, 1.0);
            let glow = ((1.0 - sy / 0.12) * 200.0) as u8;
            painter.line_segment(
                [
                    egui::pos2(center.x - line_w * 0.5, center.y),
                    egui::pos2(center.x + line_w * 0.5, center.y),
                ],
                Stroke::new(
                    2.5_f32,
                    Color32::from_rgba_unmultiplied(220, 240, 255, glow),
                ),
            );
            if sx < 0.15 {
                let r = (4.0 + (1.0 - sx / 0.15) * 5.0).max(2.0);
                painter.circle_filled(
                    center,
                    r,
                    Color32::from_rgba_unmultiplied(230, 245, 255, glow),
                );
            }
        }
    }

    if app.show_fft_debug {
        draw_fft_graph(&painter, rect, &features, color_mode, &colors);
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
    radial: &mut crate::renderer::RadialSpectrum,
    painter: &egui::Painter,
    center: Pos2,
    disc_r: f32,
    min_dim: f32,
    features: &crate::audio::AudioFeatures,
    vis: &crate::config::VisualizerSettings,
    colors: &ColorSettings,
    time: f32,
) {
    if vis.style == SpectrumStyle::Bars {
        radial.paint(
            painter, center, disc_r, min_dim, features, vis, colors, time,
        );
        return;
    }

    let spectrum = &features.spectrum;
    if spectrum.is_empty() {
        return;
    }

    let n = spectrum.len();
    let mut smoothed = spectrum.clone();
    let wrap_ring = vis.layout != SpectrumLayout::Split;

    // Soft gate: only squash bins whose neighbors are also quiet.
    // Punching a hole next to a live bin tessellates as a radial "laser".
    let gate = (0.035 + (1.0 - features.rms).clamp(0.0, 1.0) * 0.05).clamp(0.03, 0.1);
    let gated = smoothed.clone();
    for i in 0..n {
        let m = gated[i];
        if m >= gate {
            continue;
        }
        let left = if i == 0 {
            if wrap_ring {
                gated[n - 1]
            } else {
                gated[i]
            }
        } else {
            gated[i - 1]
        };
        let right = if i + 1 >= n {
            if wrap_ring {
                gated[0]
            } else {
                gated[i]
            }
        } else {
            gated[i + 1]
        };
        if left.max(right) < gate * 1.8 {
            let u = (m / gate).clamp(0.0, 1.0);
            smoothed[i] = m * u * u;
        }
    }

    let smooth_amt = (vis.smoothing * 0.42).clamp(0.0, 0.55);
    if smooth_amt > 0.02 {
        let prev = smoothed.clone();
        for i in 0..n {
            let t = i as f32 / n as f32;
            // Keep ears a bit sharper than the rest of the ring.
            let amt = if t < 0.22 || t > 0.78 {
                smooth_amt * 0.35
            } else {
                smooth_amt
            };
            let a = if i == 0 {
                if wrap_ring {
                    prev[n - 1]
                } else {
                    prev[i]
                }
            } else {
                prev[i - 1]
            };
            let b = prev[i];
            let c = if i + 1 >= n {
                if wrap_ring {
                    prev[0]
                } else {
                    prev[i]
                }
            } else {
                prev[i + 1]
            };
            let blended = b * (1.0 - amt) + (a + c) * 0.5 * amt;
            smoothed[i] = blended.max(b * 0.88);
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
    let cycle = time * vis.cycle_speed.clamp(0.0, 3.0);
    let ear_lift = disc_r * vis.ear_radius.clamp(0.0, 0.35);
    let mut pts = collect_ring_points(&smoothed, vis, features);

    // Extra outline-oriented blur. `pts` always go all the way around the disc.
    let outline_s = vis.outline_smooth.clamp(0.0, 1.0);
    if outline_s > 0.05 {
        blur_ring_mags(&mut pts, outline_s, true);
    }
    // Fill isolated dropouts so a near-zero bin next to live neighbors
    // can't form a radial spoke. Wide quiet regions (and silence) stay down.
    lift_ring_notches(&mut pts, true);

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
                cycle,
                ear_lift,
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
                cycle,
                matches!(vis.style, SpectrumStyle::SoftGlow),
                vis.layout != SpectrumLayout::Split,
                vis.outline_width.clamp(0.4, 4.5),
                ear_lift,
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
    /// 0..1 ear lobe weight — lifts radius for bass spikes.
    ear: f32,
}

fn collect_ring_points(
    smoothed: &[f32],
    vis: &crate::config::VisualizerSettings,
    features: &crate::audio::AudioFeatures,
) -> Vec<SpecPoint> {
    let n = smoothed.len().max(8);
    match vis.layout {
        SpectrumLayout::Continuous | SpectrumLayout::Mirrored => (0..n)
            .map(|i| {
                let t = (i as f32 + 0.5) / n as f32;
                let mag = if vis.layout == SpectrumLayout::Mirrored {
                    let u = if t <= 0.5 { t * 2.0 } else { (1.0 - t) * 2.0 };
                    interp_spec(smoothed, u * (n.saturating_sub(1) as f32))
                } else {
                    smoothed[i].clamp(0.0, 1.2)
                };
                SpecPoint {
                    t,
                    mag,
                    hue: t,
                    ear: 0.0,
                }
            })
            .collect(),
        SpectrumLayout::Split => {
            let cut = ((n as f32) * 0.20) as usize;
            let rest = (n - cut).max(8);
            let ear_gain = vis.ear_gain.clamp(0.2, 2.8);
            let ear_count = if vis.ear_count >= 4 { 4 } else { 2 };
            let ear_angle = vis.ear_angle.clamp(0.08, 0.42);
            let ear_width = vis.ear_width.clamp(0.035, 0.16);
            let punch = (features.bass * 0.5 + features.kick * 0.95).min(1.2);
            (0..n)
                .map(|i| {
                    let t = (i as f32 + 0.5) / n as f32;
                    let u = if t <= 0.5 { t * 2.0 } else { (1.0 - t) * 2.0 };
                    let (mag, ear) = trap_nation_mag(
                        smoothed, cut, rest, u, punch, ear_gain, ear_count, ear_angle, ear_width,
                    );
                    SpecPoint {
                        t,
                        mag,
                        hue: (0.06 + u * 0.82).clamp(0.0, 1.0),
                        ear,
                    }
                })
                .collect()
        }
    }
}

fn ear_gauss(u: f32, center: f32, width: f32) -> f32 {
    let x = (u - center) / width.max(0.02);
    (-x * x * 2.1).exp()
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
        let w = ear_width;
        ear_gauss(u, c1, w) * 0.92 + ear_gauss(u, c2, w * 0.95) * 0.78
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

/// Blend bass (bin 0) and highs (last bin) across a few samples so Full layout
/// doesn't join them as a radial spoke at 12 o'clock.
fn close_full_seam(mags: &mut [f32]) {
    let n = mags.len();
    if n < 8 {
        return;
    }
    let k = (n / 20).clamp(6, 14);
    let target = (mags[0] + mags[n - 1]) * 0.5;
    for i in 0..k {
        let w = ((k - i) as f32 / k as f32).powf(1.35);
        mags[i] = mags[i] * (1.0 - w) + target * w;
        mags[n - 1 - i] = mags[n - 1 - i] * (1.0 - w) + target * w;
    }
}

/// Raise bins that fall faster than `max_drop` below a neighbor (both directions).
fn slope_limit_down(mags: &mut [f32], wrap: bool, max_drop: f32) {
    let n = mags.len();
    if n < 2 {
        return;
    }
    let max_drop = max_drop.max(0.02);
    for i in 1..n {
        let floor = (mags[i - 1] - max_drop).max(0.0);
        if mags[i] < floor {
            mags[i] = floor;
        }
    }
    if wrap {
        let floor = (mags[n - 1] - max_drop).max(0.0);
        if mags[0] < floor {
            mags[0] = floor;
        }
    }
    for i in (0..n - 1).rev() {
        let floor = (mags[i + 1] - max_drop).max(0.0);
        if mags[i] < floor {
            mags[i] = floor;
        }
    }
    if wrap {
        let floor = (mags[0] - max_drop).max(0.0);
        if mags[n - 1] < floor {
            mags[n - 1] = floor;
        }
    }
}

fn ring_neighbor(mags: &[f32], i: usize, wrap: bool, delta: isize) -> f32 {
    let n = mags.len() as isize;
    if n <= 0 {
        return 0.0;
    }
    let j = i as isize + delta;
    let j = if wrap {
        (j.rem_euclid(n)) as usize
    } else {
        j.clamp(0, n - 1) as usize
    };
    mags[j]
}

/// Raise narrow valleys so the ring never cuts a single-bin hole.
/// Peaks stay as-is; a fully quiet ring stays quiet.
fn lift_ring_notches(pts: &mut [SpecPoint], wrap: bool) {
    let n = pts.len();
    if n < 3 {
        return;
    }
    // ~1/6 of full scale per step — steep enough for real peaks, too gentle for a needle.
    const MAX_DROP: f32 = 0.18;
    for _ in 0..3 {
        let src: Vec<f32> = pts.iter().map(|p| p.mag).collect();
        for i in 0..n {
            let a = ring_neighbor(&src, i, wrap, -1);
            let c = ring_neighbor(&src, i, wrap, 1);
            let floor = (a.max(c) - MAX_DROP).max(0.0);
            if pts[i].mag < floor {
                pts[i].mag = floor;
            }
        }
    }
}

fn blur_ring_mags(pts: &mut [SpecPoint], amount: f32, wrap: bool) {
    if pts.len() < 3 {
        return;
    }
    let amt = amount.clamp(0.0, 1.0) * 0.55;
    let prev: Vec<f32> = pts.iter().map(|p| p.mag).collect();
    let n = pts.len();
    for i in 0..n {
        let (a, c) = if wrap {
            (
                prev[(i + n - 1) % n],
                prev[(i + 1) % n],
            )
        } else {
            (
                prev[i.saturating_sub(1)],
                prev[(i + 1).min(n - 1)],
            )
        };
        let b = prev[i];
        // Preserve ear peaks a bit so lobes stay readable.
        let keep = 1.0 - amt * (1.0 - pts[i].ear * 0.55);
        pts[i].mag = b * keep + (a + c) * 0.5 * (1.0 - keep);
    }
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

#[allow(dead_code)]
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
    cycle: f32,
    ear_lift: f32,
    pts: &[SpecPoint],
) {
    for p in pts {
        let mag = p.mag.clamp(0.0, 1.45);
        if !mag.is_finite() {
            continue;
        }
        // Keep a short stub so quiet bins stay in the ring instead of leaving gaps.
        let len = (mag * max_len).max((max_len * 0.05).max(2.5));
        let r0 = base_r + ear_lift * p.ear;
        let inner = ring_pos(center, p.t, r0);
        let outer = ring_pos(center, p.t, r0 + len);
        let color = spectrum_color(p.hue, intensity, mode, colors, cycle);
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
    cycle: f32,
    soft_glow: bool,
    rainbow_wrap: bool,
    outline_width: f32,
    ear_lift: f32,
    pts: &[SpecPoint],
) {
    if pts.len() < 2 {
        return;
    }
    let n = pts.len();

    // Soft-close Full seam: blend first/last mags so bass/highs don't meet as a spoke.
    let mut mags: Vec<f32> = pts
        .iter()
        .map(|p| p.mag.clamp(0.0, 1.45))
        .collect();
    if rainbow_wrap && n >= 8 {
        close_full_seam(&mut mags);
    }
    slope_limit_down(&mut mags, true, 0.22);

    // Thin resting ring so quiet frequency stretches stay connected.
    let min_len = (max_len * 0.05).max(2.5);

    for i in 0..n {
        let j = (i + 1) % n;
        let a = pts[i];
        let b = pts[j];
        let len0 = (mags[i] * max_len).max(min_len);
        let len1 = (mags[j] * max_len).max(min_len);
        let r0 = base_r + ear_lift * a.ear;
        let r1 = base_r + ear_lift * b.ear;
        let inner0 = ring_pos(center, a.t, r0);
        let inner1 = ring_pos(center, b.t, r1);
        let outer0 = ring_pos(center, a.t, r0 + len0);
        let outer1 = ring_pos(center, b.t, r1 + len1);
        let hue = if rainbow_wrap && j == 0 {
            1.0
        } else {
            (a.hue + b.hue) * 0.5
        };
        let color = spectrum_color(hue, intensity, mode, colors, cycle);

        if soft_glow && glow > 0.05 {
            let gc = with_alpha(color, (32.0 * glow) as u8);
            painter.add(Shape::convex_polygon(
                vec![
                    inner0,
                    ring_pos(center, a.t, r0 + len0 * 1.04),
                    ring_pos(center, b.t, r1 + len1 * 1.04),
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
        // Slightly thicker stroke with a soft under-glow for a cleaner rim.
        if outline_width > 0.6 {
            painter.line_segment(
                [outer0, outer1],
                Stroke::new(
                    outline_width * 2.1,
                    with_alpha(color, 48),
                ),
            );
        }
        painter.line_segment(
            [outer0, outer1],
            Stroke::new(outline_width, with_alpha(color, 235)),
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

fn draw_playlist_item(
    painter: &egui::Painter,
    app: &mut LiveVisualizerApp,
    idx: usize,
    center: Pos2,
    disc_r: f32,
    logo_angle: f32,
    features: &crate::audio::AudioFeatures,
    stage: &crate::config::StageSettings,
    time: f32,
    dt: f32,
    sx: f32,
    sy: f32,
    alpha: f32,
) {
    let path = app
        .settings
        .logo_paths
        .get(idx)
        .map(|s| s.as_str())
        .unwrap_or("");
    let a = alpha.clamp(0.0, 1.0);

    if crate::config::is_center_scope(path) {
        draw_oscilloscope(painter, center, disc_r, features, stage, time, sx, sy, a);
        return;
    }
    if crate::config::is_center_spectrum(path) {
        draw_center_spectrum(
            painter,
            center,
            disc_r,
            features,
            stage,
            time,
            dt,
            &mut app.center_bars_smooth,
            sx,
            sy,
            a,
        );
        return;
    }

    let Some(Some(cur)) = app.logos.get(idx) else {
        return;
    };
    let style = app.settings.logo_style(idx);
    let logo_center = egui::pos2(
        center.x + style.offset_x * disc_r,
        center.y + style.offset_y * disc_r,
    );
    let from_idx = app.logo_from_index.min(app.playlist_len().saturating_sub(1));
    let base = fit_rect(
        egui::Rect::from_center_size(
            logo_center,
            egui::vec2(disc_r * 2.0 * style.size, disc_r * 2.0 * style.size),
        ),
        cur.size,
    );
    let logo_rect = egui::Rect::from_center_size(
        base.center(),
        egui::vec2(base.width() * sx, base.height() * sy),
    );
    let mut tint = logo_style_tint(&style);
    tint = Color32::from_rgba_unmultiplied(
        tint.r(),
        tint.g(),
        tint.b(),
        ((tint.a() as f32) * a) as u8,
    );
    let switch_g = app.logo_switch_glitch_t * stage.logo_glitch_amount.clamp(0.0, 1.5);
    let beat_g = app.logo_beat_glitch_t * stage.logo_beat_glitch_amount.clamp(0.0, 1.5);
    let cur_tex = cur.texture.id();
    let prev_tex = app
        .logos
        .get(from_idx)
        .and_then(|o| o.as_ref())
        .filter(|_| from_idx != idx && !app.crt_phase.is_busy())
        .map(|t| t.texture.id());
    if switch_g > 0.04 && !app.crt_phase.is_busy() {
        paint_logo_glitch(
            painter, cur_tex, prev_tex, logo_rect, logo_angle, tint, switch_g,
        );
    } else if beat_g > 0.04 && !app.crt_phase.is_busy() {
        paint_logo_glitch(painter, cur_tex, None, logo_rect, logo_angle, tint, beat_g);
    } else {
        paint_rotated_image(painter, cur_tex, logo_rect, logo_angle, tint);
    }
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

/// Soft-edged time-domain scope inside the disc (no hard circle clamps).
fn draw_oscilloscope(
    painter: &egui::Painter,
    center: Pos2,
    disc_r: f32,
    features: &crate::audio::AudioFeatures,
    stage: &crate::config::StageSettings,
    time: f32,
    sx: f32,
    sy: f32,
    alpha: f32,
) {
    let wave = &features.waveform;
    if wave.len() < 2 || disc_r < 8.0 || alpha < 0.02 {
        return;
    }

    let gain = stage.scope_gain.clamp(0.1, 4.0);
    let thickness = stage.scope_thickness.clamp(0.5, 6.0);
    let n = wave.len();

    // Dual-pass smooth for a cleaner CRT-ish line.
    let mut smooth = vec![0.0_f32; n];
    for i in 0..n {
        let a = wave[i.saturating_sub(2)];
        let b = wave[i.saturating_sub(1)];
        let c = wave[i];
        let d = wave[(i + 1).min(n - 1)];
        let e = wave[(i + 2).min(n - 1)];
        smooth[i] = a * 0.05 + b * 0.2 + c * 0.5 + d * 0.2 + e * 0.05;
    }

    let span = disc_r * 0.86 * sx.clamp(0.02, 1.0);
    let amp = disc_r * 0.58 * gain * sy.clamp(0.02, 1.0);
    let cycle = time * stage.scope_cycle_speed.clamp(0.0, 3.0);
    let alpha = alpha.clamp(0.0, 1.0);

    painter.line_segment(
        [
            egui::pos2(center.x - span * 0.9, center.y),
            egui::pos2(center.x + span * 0.9, center.y),
        ],
        Stroke::new(
            1.0_f32,
            Color32::from_rgba_unmultiplied(255, 255, 255, (18.0 * alpha) as u8),
        ),
    );

    for i in 0..n - 1 {
        let t0 = i as f32 / (n - 1) as f32;
        let t1 = (i + 1) as f32 / (n - 1) as f32;
        let w0 = ((t0 * std::f32::consts::PI).sin()).powi(2);
        let w1 = ((t1 * std::f32::consts::PI).sin()).powi(2);
        let w = ((w0 + w1) * 0.5).clamp(0.0, 1.0);
        if w < 0.015 {
            continue;
        }

        let dx0 = (t0 - 0.5) * 2.0 * span;
        let dx1 = (t1 - 0.5) * 2.0 * span;
        let dy0 = -smooth[i].clamp(-1.0, 1.0) * amp * w0;
        let dy1 = -smooth[i + 1].clamp(-1.0, 1.0) * amp * w1;
        let p0 = egui::pos2(center.x + dx0, center.y + dy0);
        let p1 = egui::pos2(center.x + dx1, center.y + dy1);

        let mid_t = (t0 + t1) * 0.5;
        let rgb = effect_rgb(
            stage.scope_color_mode,
            stage.scope_color,
            stage.scope_color_b,
            mid_t,
            cycle,
        );
        let a = (w.powf(0.65) * 245.0 * alpha) as u8;
        let color = Color32::from_rgba_unmultiplied(
            (rgb.r * 255.0) as u8,
            (rgb.g * 255.0) as u8,
            (rgb.b * 255.0) as u8,
            a,
        );
        if thickness > 1.0 {
            painter.line_segment(
                [p0, p1],
                Stroke::new(
                    thickness * 2.6,
                    Color32::from_rgba_unmultiplied(
                        (rgb.r * 255.0) as u8,
                        (rgb.g * 255.0) as u8,
                        (rgb.b * 255.0) as u8,
                        (w * 42.0 * alpha) as u8,
                    ),
                ),
            );
        }
        painter.line_segment([p0, p1], Stroke::new(thickness * (0.5 + 0.5 * w), color));
    }
}

/// Apple call / Music-style bars: bass in the center, highs on the outsides,
/// left-right symmetric, mirrored up/down from the midline.
fn draw_center_spectrum(
    painter: &egui::Painter,
    center: Pos2,
    disc_r: f32,
    features: &crate::audio::AudioFeatures,
    stage: &crate::config::StageSettings,
    time: f32,
    dt: f32,
    smooth: &mut Vec<f32>,
    sx: f32,
    sy: f32,
    alpha: f32,
) {
    let spec = &features.spectrum;
    if spec.is_empty() || disc_r < 8.0 || alpha < 0.02 {
        return;
    }

    // Odd count so there's a true center bar (FaceTime / Music vibe).
    let bars = 21_usize;
    if smooth.len() != bars {
        smooth.resize(bars, 0.0);
    }

    let gain = stage.spectrum_gain.clamp(0.1, 4.0);
    let thickness = stage.spectrum_thickness.clamp(0.5, 6.0);
    let span = disc_r * 0.78 * sx.clamp(0.02, 1.0);
    let max_h = disc_r * 0.48 * gain * sy.clamp(0.02, 1.0);
    let cycle = time * stage.spectrum_cycle_speed.clamp(0.0, 3.0);
    let alpha = alpha.clamp(0.0, 1.0);
    let mid = (bars as f32 - 1.0) * 0.5;

    // Overall voice/music level — keeps the whole row alive like a call meter.
    let voice = (features.rms * 0.45 + features.mid * 0.35 + features.bass * 0.25
        + features.low_mid * 0.15)
        .clamp(0.0, 1.0);

    let attack = 1.0 - (-dt * 18.0).exp();
    let release = 1.0 - (-dt * 7.0).exp();

    for i in 0..bars {
        // 0 at center → 1 at either edge. Bass lives in the middle.
        let dist = ((i as f32 - mid).abs() / mid).clamp(0.0, 1.0);
        // Sample spectrum by distance-from-center (not left→right song bias).
        let bin_f = dist.powf(0.85) * (spec.len().saturating_sub(1) as f32);
        let bin = bin_f as usize;
        let frac = bin_f - bin as f32;
        let a = spec.get(bin).copied().unwrap_or(0.0);
        let b = spec.get(bin + 1).copied().unwrap_or(a);
        let band = (a + (b - a) * frac).clamp(0.0, 1.2);

        // Center accent + shared voice energy so it doesn't lean with the mix.
        let center_boost = 1.0 - dist * 0.35;
        let target = (band * 0.7 * center_boost + voice * 0.45 * (0.55 + 0.45 * center_boost))
            .clamp(0.0, 1.15)
            .powf(0.9);

        let cur = smooth[i];
        let rate = if target > cur { attack } else { release };
        smooth[i] = cur + (target - cur) * rate;
    }

    // Even bar geometry — identical widths, centered in the disc.
    let step = (span * 2.0) / bars as f32;
    let bar_w = (step * 0.62).clamp(2.0, 14.0);
    let radius = (bar_w * 0.45).clamp(1.0, 6.0);

    for i in 0..bars {
        let mag = smooth[i];
        let h = (mag * max_h).max(0.0);
        // Tiny resting height so bars never fully disappear (call-meter feel).
        let h = h.max(bar_w * 0.35 * (0.25 + 0.75 * voice) * sy.clamp(0.02, 1.0));
        if h < 0.5 {
            continue;
        }

        let t = i as f32 / (bars - 1).max(1) as f32;
        let mid_x = center.x - span + step * (i as f32 + 0.5);
        let half_w = bar_w * 0.5;

        let rgb = effect_rgb(
            stage.spectrum_color_mode,
            stage.spectrum_color,
            stage.spectrum_color_b,
            t,
            cycle,
        );
        let a = ((170.0 + 75.0 * mag.min(1.0)) * alpha) as u8;
        let fill = Color32::from_rgba_unmultiplied(
            (rgb.r * 255.0) as u8,
            (rgb.g * 255.0) as u8,
            (rgb.b * 255.0) as u8,
            a,
        );
        let glow = Color32::from_rgba_unmultiplied(
            (rgb.r * 255.0) as u8,
            (rgb.g * 255.0) as u8,
            (rgb.b * 255.0) as u8,
            (36.0 * alpha) as u8,
        );

        // One continuous capsule through the midline (up + down).
        let rect = egui::Rect::from_min_max(
            egui::pos2(mid_x - half_w, center.y - h),
            egui::pos2(mid_x + half_w, center.y + h),
        );
        if thickness > 1.2 {
            painter.rect_filled(rect.expand(1.4), radius + 1.0, glow);
        }
        painter.rect_filled(rect, radius, fill);
    }
}

fn effect_rgb(
    mode: ScopeColorMode,
    color_a: RgbColor,
    color_b: RgbColor,
    t: f32,
    cycle: f32,
) -> RgbColor {
    match mode {
        ScopeColorMode::Solid => color_a,
        ScopeColorMode::Rainbow => {
            let (r, g, b) = crate::config::neon_rainbow(t);
            RgbColor::new(r, g, b)
        }
        ScopeColorMode::Gradient => color_a.lerp(color_b, t),
        ScopeColorMode::Cycle => {
            let (r, g, b) = crate::config::neon_rainbow((t + cycle).fract());
            RgbColor::new(r, g, b)
        }
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
        painter.rect_filled(bar, 0.0, spectrum_color(i as f32 / n as f32, 0.9, mode, colors, 0.0));
    }
}

fn spectrum_color(
    t: f32,
    intensity: f32,
    mode: ColorMode,
    colors: &ColorSettings,
    cycle: f32,
) -> Color32 {
    let rgb = colors.sample_cycled(mode, t, intensity, cycle);
    Color32::from_rgb(
        (rgb.r * 255.0) as u8,
        (rgb.g * 255.0) as u8,
        (rgb.b * 255.0) as u8,
    )
}

fn with_alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}
