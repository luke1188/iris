//! egui settings panel and stage preview.

use super::media::{cover_rect, fit_rect};
use super::state::LiveVisualizerApp;
use crate::config::{BeatSettings, BeatMode, ColorMode, LogoMotion, SpectrumLayout, SpectrumStyle};
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
        ui.heading(RichText::new("Live Visualizer").color(Color32::from_rgb(0, 220, 255)));
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
                    section_hint(ui, "Audio input, levels, and beat bounce");
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
                    section_hint(ui, "Per-band gains for loud rooms");
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
            if ui.button("Refresh").on_hover_text("Reload presets folder").clicked() {
                app.refresh_presets();
            }
            ui.label(
                RichText::new(format!(
                    "Active: {}",
                    app.settings.last_preset.as_deref().unwrap_or("none")
                ))
                .small()
                .color(Color32::GRAY),
            );
        });
        ui.label(
            RichText::new(format!(
                "Saves to: {}",
                crate::config::presets_dir().display()
            ))
            .small()
            .color(Color32::DARK_GRAY),
        );

        ui.add_space(4.0);
        let names = app.preset_names.clone();
        egui::Grid::new("preset_grid")
            .num_columns(2)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                for name in &names {
                    let selected = app.settings.last_preset.as_deref() == Some(name.as_str());
                    if ui.selectable_label(selected, name).clicked() {
                        app.apply_preset_by_name(name);
                    }
                    ui.end_row();
                }
            });

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Save as");
            ui.text_edit_singleline(&mut app.preset_save_name);
            if ui.button("Save").clicked() {
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
        ui.add_space(4.0);

        let mut changed = false;
        {
            let b = &mut app.settings.beat;
            ui.label(
                RichText::new("Kick only = drums. Bass level = 808 sustain too.")
                    .small()
                    .color(Color32::GRAY),
            );
            ui.horizontal_wrapped(|ui| {
                for (mode, label) in [
                    (BeatMode::BassKick, "Bass kick"),
                    (BeatMode::BassLevel, "Bass level"),
                    (BeatMode::KickOnly, "Kick only"),
                    (BeatMode::FullMix, "Full mix"),
                ] {
                    changed |= ui.selectable_value(&mut b.mode, mode, label).changed();
                }
            });
            ui.add_space(4.0);
            changed |= slider(ui, "Sensitivity", &mut b.sensitivity, 0.4..=3.5).changed();
            changed |= slider(ui, "Kick punch", &mut b.kick_sensitivity, 0.3..=3.0).changed();
            changed |= slider(ui, "Bass weight", &mut b.bass_weight, 0.0..=2.5).changed();
            changed |= slider(ui, "RMS weight", &mut b.rms_weight, 0.0..=2.0).changed();
            changed |= slider(ui, "Cooldown", &mut b.cooldown, 0.16..=0.4).changed();
        }

        ui.add_space(6.0);
        ui.label(RichText::new("Quick setups").small().strong());
        ui.horizontal_wrapped(|ui| {
            if ui.button("Bass only").clicked() {
                app.apply_beat_preset(BeatSettings::preset_bass_only());
            }
            if ui.button("Kick only").clicked() {
                app.apply_beat_preset(BeatSettings::preset_kick_only());
            }
            if ui.button("Bass level").clicked() {
                app.apply_beat_preset(BeatSettings::preset_bass_level());
            }
            if ui.button("Sensitive").clicked() {
                app.apply_beat_preset(BeatSettings::preset_sensitive());
            }
            if ui.button("Tight").clicked() {
                app.apply_beat_preset(BeatSettings::preset_tight());
            }
        });
        if ui
            .button("Reset kick bounce")
            .on_hover_text("Sets Bass bounce / Beat bounce to values that read on a kick")
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
            RichText::new("Loud bar tip: raise Threshold, lower Bass")
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
            changed |= slider(ui, "Bass tilt", &mut t.spectrum_bass_tilt, 0.1..=1.5).changed();
            ui.add_space(4.0);
            changed |= slider(ui, "Attack", &mut t.attack, 0.05..=1.0).changed();
            changed |= slider(ui, "Release", &mut t.release, 0.02..=0.6).changed();
        }

        ui.horizontal(|ui| {
            if ui.button("Reset").clicked() {
                app.analyzer.tuning.reset_defaults();
                changed = true;
            }
            if ui.button("Bar venue").clicked() {
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
        level_bar(ui, "RMS", app.features.rms, Color32::from_rgb(180, 180, 200));
        level_bar(ui, "Bass", app.features.bass, Color32::from_rgb(255, 60, 100));
        level_bar(ui, "Low Mid", app.features.low_mid, Color32::from_rgb(255, 140, 60));
        level_bar(ui, "Mid", app.features.mid, Color32::from_rgb(255, 220, 60));
        level_bar(ui, "High Mid", app.features.high_mid, Color32::from_rgb(60, 220, 120));
        level_bar(ui, "High", app.features.high, Color32::from_rgb(60, 180, 255));
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
            ui.add_space(4.0);
            ui.label(RichText::new("Color").small().strong());
            ui.horizontal_wrapped(|ui| {
                for (mode, label) in [
                    (ColorMode::Rgb, "RGB"),
                    (ColorMode::Mono, "Mono"),
                    (ColorMode::Cyan, "Cyan"),
                    (ColorMode::Amber, "Amber"),
                    (ColorMode::Magenta, "Magenta"),
                ] {
                    changed |= ui.selectable_value(&mut v.color_mode, mode, label).changed();
                }
            });
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
            if ui.button("Choose logo…").clicked() {
                app.pick_logo(ui.ctx());
            }
            if ui.button("Clear").clicked() {
                app.clear_logo();
            }
        });
        if let Some(ref path) = app.settings.logo_path {
            ui.label(
                RichText::new(truncate_path(path, 42))
                    .small()
                    .color(Color32::GRAY),
            );
        }

        let mut changed = false;
        {
            let s = &mut app.settings.stage;
            ui.add_space(4.0);
            ui.label(RichText::new("Size & bounce").small().strong());
            changed |= slider(ui, "Disc size", &mut s.disc_radius, 0.06..=0.35).changed();
            changed |= slider(ui, "Logo size", &mut s.logo_size, 0.25..=1.0).changed();
            changed |= slider(ui, "Bass bounce", &mut s.bass_pulse, 0.0..=0.35).changed();
            changed |= slider(ui, "Beat bounce", &mut s.beat_pulse, 0.0..=0.45).changed();

            ui.add_space(6.0);
            ui.label(RichText::new("Motion").small().strong());
            ui.horizontal_wrapped(|ui| {
                for (mode, label) in [
                    (LogoMotion::None, "None"),
                    (LogoMotion::Spin, "Spin"),
                    (LogoMotion::BeatSpin, "Beat spin"),
                    (LogoMotion::Wobble, "Wobble"),
                    (LogoMotion::Pendulum, "Pendulum"),
                ] {
                    if ui.selectable_value(&mut s.logo_motion, mode, label).changed() {
                        changed = true;
                    }
                }
            });
            if s.logo_motion != LogoMotion::None {
                changed |= slider(ui, "Speed", &mut s.logo_spin_speed, 0.02..=1.5)
                    .on_hover_text("Spin: revolutions/sec · Wobble/Pendulum: cycle rate")
                    .changed();
                if matches!(s.logo_motion, LogoMotion::Wobble | LogoMotion::Pendulum) {
                    changed |= slider(ui, "Amount", &mut s.logo_motion_amount, 0.05..=1.0)
                        .changed();
                }
            }
            changed |= ui
                .checkbox(&mut s.disc_ticks, "Disc tick marks")
                .on_hover_text("Subtle marks on the black disc that rotate with the logo")
                .changed();

            ui.add_space(6.0);
            ui.label(RichText::new("Alignment").small().strong());
            changed |= slider(ui, "Offset X", &mut s.logo_offset_x, -0.45..=0.45)
                .on_hover_text("Negative = left, positive = right")
                .changed();
            changed |= slider(ui, "Offset Y", &mut s.logo_offset_y, -0.45..=0.45)
                .on_hover_text("Negative = up, positive = down")
                .changed();

            ui.horizontal(|ui| {
                let step = 0.02_f32;
                if ui.button("←").clicked() {
                    s.logo_offset_x = (s.logo_offset_x - step).max(-0.45);
                    changed = true;
                }
                if ui.button("→").clicked() {
                    s.logo_offset_x = (s.logo_offset_x + step).min(0.45);
                    changed = true;
                }
                if ui.button("↑").clicked() {
                    s.logo_offset_y = (s.logo_offset_y - step).max(-0.45);
                    changed = true;
                }
                if ui.button("↓").clicked() {
                    s.logo_offset_y = (s.logo_offset_y + step).min(0.45);
                    changed = true;
                }
                if ui.button("Center").clicked() {
                    s.logo_offset_x = 0.0;
                    s.logo_offset_y = 0.0;
                    changed = true;
                }
            });

            ui.add_space(6.0);
            ui.label(RichText::new("Color").small().strong());
            changed |= slider(ui, "Brightness", &mut s.logo_brightness, 0.2..=1.8).changed();
            changed |= slider(ui, "Opacity", &mut s.logo_opacity, 0.15..=1.0).changed();
            changed |= slider(ui, "Tint R", &mut s.logo_tint_r, 0.0..=1.5).changed();
            changed |= slider(ui, "Tint G", &mut s.logo_tint_g, 0.0..=1.5).changed();
            changed |= slider(ui, "Tint B", &mut s.logo_tint_b, 0.0..=1.5).changed();

            // Color preview swatch
            let preview = logo_tint_color(s);
            let (swatch, _) =
                ui.allocate_exact_size(egui::vec2(ui.available_width(), 18.0), Sense::hover());
            ui.painter().rect_filled(swatch, 3.0, preview);

            ui.add_space(4.0);
            ui.label(RichText::new("Tint presets").small().strong());
            ui.horizontal_wrapped(|ui| {
                if ui.button("White").clicked() {
                    set_logo_tint(s, 1.0, 1.0, 1.0, 1.0);
                    changed = true;
                }
                if ui.button("Warm").clicked() {
                    set_logo_tint(s, 1.15, 0.95, 0.75, 1.05);
                    changed = true;
                }
                if ui.button("Cool").clicked() {
                    set_logo_tint(s, 0.75, 0.95, 1.2, 1.05);
                    changed = true;
                }
                if ui.button("Cyan").clicked() {
                    set_logo_tint(s, 0.45, 1.05, 1.25, 1.1);
                    changed = true;
                }
                if ui.button("Magenta").clicked() {
                    set_logo_tint(s, 1.25, 0.45, 1.1, 1.1);
                    changed = true;
                }
                if ui.button("Gold").clicked() {
                    set_logo_tint(s, 1.25, 0.95, 0.4, 1.1);
                    changed = true;
                }
            });
        }
        if changed {
            app.mark_settings_dirty();
        }
    });
}

fn set_logo_tint(s: &mut crate::config::StageSettings, r: f32, g: f32, b: f32, bright: f32) {
    s.logo_tint_r = r;
    s.logo_tint_g = g;
    s.logo_tint_b = b;
    s.logo_brightness = bright;
    s.logo_opacity = 1.0;
}

fn logo_tint_color(s: &crate::config::StageSettings) -> Color32 {
    let b = s.logo_brightness.clamp(0.0, 2.0);
    let a = (s.logo_opacity.clamp(0.0, 1.0) * 255.0) as u8;
    Color32::from_rgba_unmultiplied(
        (s.logo_tint_r.clamp(0.0, 2.0) * b * 255.0).min(255.0) as u8,
        (s.logo_tint_g.clamp(0.0, 2.0) * b * 255.0).min(255.0) as u8,
        (s.logo_tint_b.clamp(0.0, 2.0) * b * 255.0).min(255.0) as u8,
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

    app.particles.draw(&painter, color_mode);

    draw_spectrum(&painter, center, disc_r, min_dim, features, &vis);

    // Clean black disc — no colored glow halo behind the logo.
    painter.circle_filled(center, disc_r, Color32::from_rgb(4, 4, 6));
    painter.circle_stroke(
        center,
        disc_r,
        Stroke::new(1.5_f32, Color32::from_rgb(28, 28, 36)),
    );

    let logo_angle = app.logo_draw_angle();
    if stage.disc_ticks {
        draw_disc_ticks(&painter, center, disc_r, logo_angle);
    }

    if let Some(ref logo) = app.logo {
        let logo_center = egui::pos2(
            center.x + stage.logo_offset_x * disc_r,
            center.y + stage.logo_offset_y * disc_r,
        );
        let logo_bounds = egui::Rect::from_center_size(
            logo_center,
            egui::vec2(disc_r * 2.0 * stage.logo_size, disc_r * 2.0 * stage.logo_size),
        );
        let logo_rect = fit_rect(logo_bounds, logo.size);
        let tint = logo_tint_color(&stage);
        paint_rotated_image(
            &painter,
            logo.texture.id(),
            logo_rect,
            logo_angle,
            tint,
        );
    }

    if app.show_fft_debug {
        draw_fft_graph(&painter, rect, features, color_mode);
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
    pts: &[SpecPoint],
) {
    for p in pts {
        let len = p.mag.clamp(0.0, 1.45) * max_len;
        let inner = ring_pos(center, p.t, base_r);
        let outer = ring_pos(center, p.t, base_r + len);
        let color = spectrum_color(p.hue, intensity, mode);
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
        let color = spectrum_color(hue, intensity, mode);

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
        painter.rect_filled(bar, 0.0, spectrum_color(i as f32 / n as f32, 0.9, mode));
    }
}

fn spectrum_color(t: f32, intensity: f32, mode: ColorMode) -> Color32 {
    let v = intensity.clamp(0.4, 1.0);
    match mode {
        ColorMode::Rgb => {
            // Smooth neon rainbow: red → yellow → green → cyan → blue → magenta → red.
            // Cosine-interpolated stops (no hard HSV segment edges).
            let (r, g, b) = neon_rainbow(t.fract());
            Color32::from_rgb((r * v * 255.0) as u8, (g * v * 255.0) as u8, (b * v * 255.0) as u8)
        }
        ColorMode::Mono => {
            let g = (150.0 + 95.0 * v) as u8;
            Color32::from_rgb(g, g, g)
        }
        ColorMode::Cyan => Color32::from_rgb(50, (210.0 * v) as u8, 255),
        ColorMode::Amber => Color32::from_rgb(255, (170.0 * v) as u8, 55),
        ColorMode::Magenta => Color32::from_rgb(255, 55, (190.0 * v) as u8),
    }
}

/// Piecewise cosine blend across neon stops for a continuous ring (seamless at t=0/1).
fn neon_rainbow(t: f32) -> (f32, f32, f32) {
    // Stops evenly around the circle
    const STOPS: [(f32, f32, f32); 6] = [
        (1.00, 0.20, 0.35), // red / pink
        (1.00, 0.75, 0.15), // amber
        (0.25, 1.00, 0.40), // green
        (0.15, 0.90, 1.00), // cyan
        (0.35, 0.35, 1.00), // blue
        (0.95, 0.25, 0.95), // magenta
    ];
    let n = STOPS.len() as f32;
    let x = t.fract() * n;
    let i = x.floor() as usize % STOPS.len();
    let j = (i + 1) % STOPS.len();
    let f = x - x.floor();
    let f = (1.0 - (f * std::f32::consts::PI).cos()) * 0.5; // smoothstep-ish
    let a = STOPS[i];
    let b = STOPS[j];
    (
        a.0 + (b.0 - a.0) * f,
        a.1 + (b.1 - a.1) * f,
        a.2 + (b.2 - a.2) * f,
    )
}

fn with_alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}
