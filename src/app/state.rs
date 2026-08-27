//! Shared application state and frame update loop.

use crate::audio::{AudioCapture, AudioDeviceInfo, AudioFeatures, Analyzer};
use crate::config::{
    list_presets, load_preset, load_settings, sanitize_preset_name, save_preset, save_settings,
    AppSettings, LogoMotion, LogoStyle, StageSettings,
};
use crate::display::{is_fullscreen, toggle_fullscreen};
use crate::renderer::ParticleSystem;
use eframe::egui;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use super::media::{self, LoadedTexture};
use super::ui;

#[derive(Debug, Clone, Default)]
pub struct FrameTiming {
    pub fps: f32,
    pub frame_ms: f32,
    pub audio_ms: f32,
}

pub struct LiveVisualizerApp {
    pub settings: AppSettings,
    pub capture: AudioCapture,
    pub analyzer: Analyzer,
    pub features: AudioFeatures,
    pub devices: Vec<AudioDeviceInfo>,
    pub selected_device: Option<String>,
    pub show_settings: bool,
    pub settings_popout: bool,
    pub controls_tab: usize,
    pub show_debug: bool,
    pub show_fft_debug: bool,
    pub fullscreen: bool,
    pub timing: FrameTiming,
    pub error_message: Option<String>,
    pub status_message: Option<String>,
    pub background: Option<LoadedTexture>,
    pub logos: Vec<LoadedTexture>,
    pub logo_index: usize,
    pub logo_from_index: usize,
    /// Hold countdown until next logo (seconds).
    pub logo_hold_left: f32,
    /// Glitch envelope for logo→logo transitions only.
    pub logo_switch_glitch_t: f32,
    /// Glitch envelope for beat hits only (independent of transitions).
    pub logo_beat_glitch_t: f32,
    last_beat_for_glitch: f32,
    pub particles: ParticleSystem,
    pub preset_names: Vec<String>,
    pub preset_save_name: String,
    pub settings_dirty: bool,
    pub shutting_down: bool,
    /// Accumulated logo rotation angle (radians).
    pub logo_angle: f32,
    /// Phase for wobble / pendulum (radians).
    pub logo_motion_phase: f32,
    last_frame: Instant,
    frame_count: u32,
    fps_timer: Instant,
    save_timer: Instant,
    pub features_shared: Arc<parking_lot::Mutex<AudioFeatures>>,
}

impl LiveVisualizerApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut style = (*cc.egui_ctx.style()).clone();
        style.visuals = egui::Visuals::dark();
        style.visuals.window_fill = egui::Color32::from_rgb(12, 12, 16);
        style.visuals.panel_fill = egui::Color32::from_rgb(16, 16, 22);
        style.visuals.override_text_color = Some(egui::Color32::from_rgb(220, 220, 230));
        cc.egui_ctx.set_style(style);

        let settings = load_settings().unwrap_or_default();
        let mut capture = AudioCapture::new();
        let devices = capture.list_devices();

        let selected = settings
            .last_audio_device
            .clone()
            .or_else(|| devices.first().map(|d| d.id.clone()));

        let mut error_message = None;
        if let Some(ref id) = selected {
            // Soft-fail: don't abort app startup if device is briefly busy after last run
            match capture.start(id) {
                Ok(()) => {}
                Err(e) => {
                    log::warn!("Initial audio start failed ({e}); retrying once…");
                    std::thread::sleep(std::time::Duration::from_millis(150));
                    if let Err(e2) = capture.start(id) {
                        error_message = Some(format!(
                            "Audio busy/unavailable ({e2}). Pick a device or relaunch."
                        ));
                        log::error!("{error_message:?}");
                    }
                }
            }
        }

        let sample_rate = capture.sample_rate().unwrap_or(48_000);
        let mut analyzer = Analyzer::new(sample_rate);
        analyzer.set_tuning(settings.tuning.clone());
        analyzer.beat_mut().apply_settings(&settings.beat);

        let particles = ParticleSystem::new(settings.particles.clone());
        let preset_names = list_presets();
        let preset_save_name = settings
            .last_preset
            .clone()
            .unwrap_or_else(|| "custom".into());

        let mut app = Self {
            settings,
            capture,
            analyzer,
            features: AudioFeatures::default(),
            devices,
            selected_device: selected,
            show_settings: true,
            settings_popout: false,
            controls_tab: 0,
            show_debug: true,
            show_fft_debug: false,
            fullscreen: false,
            timing: FrameTiming::default(),
            error_message,
            status_message: None,
            background: None,
            logos: Vec::new(),
            logo_index: 0,
            logo_from_index: 0,
            logo_hold_left: 8.0,
            logo_switch_glitch_t: 0.0,
            logo_beat_glitch_t: 0.0,
            last_beat_for_glitch: 0.0,
            particles,
            preset_names,
            preset_save_name,
            settings_dirty: false,
            shutting_down: false,
            logo_angle: 0.0,
            logo_motion_phase: 0.0,
            last_frame: Instant::now(),
            frame_count: 0,
            fps_timer: Instant::now(),
            save_timer: Instant::now(),
            features_shared: Arc::new(parking_lot::Mutex::new(AudioFeatures::default())),
        };

        if let Some(ref path) = app.settings.background_path.clone() {
            if let Err(e) = app.load_background_with_ctx(&cc.egui_ctx, PathBuf::from(path)) {
                log::warn!("Could not restore background: {e}");
            }
        }
        if !app.settings.logo_paths.is_empty() {
            let paths = app.settings.logo_paths.clone();
            for (i, path) in paths.iter().enumerate() {
                if let Err(e) = app.push_logo_with_ctx(&cc.egui_ctx, PathBuf::from(path), i == 0) {
                    log::warn!("Could not restore logo {path}: {e}");
                }
            }
            app.logo_hold_left = app.settings.stage.logo_hold_secs.max(1.0);
        }

        app
    }

    pub fn stage(&self) -> &StageSettings {
        &self.settings.stage
    }

    pub fn refresh_devices(&mut self) {
        self.devices = self.capture.list_devices();
    }

    pub fn select_device(&mut self, device_id: &str) {
        match self.capture.start(device_id) {
            Ok(()) => {
                self.selected_device = Some(device_id.to_string());
                self.settings.last_audio_device = Some(device_id.to_string());
                self.mark_settings_dirty();
                let sr = self.capture.sample_rate().unwrap_or(48_000);
                self.analyzer.set_sample_rate(sr);
                self.error_message = None;
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to start device: {e}"));
                log::error!("{}", self.error_message.as_ref().unwrap());
            }
        }
    }

    pub fn apply_beat_from_ui(&mut self) {
        self.analyzer.beat_mut().apply_settings(&self.settings.beat);
        self.mark_settings_dirty();
    }

    pub fn apply_beat_preset(&mut self, preset: crate::config::BeatSettings) {
        self.settings.beat = preset;
        self.apply_beat_from_ui();
    }

    pub fn mark_settings_dirty(&mut self) {
        self.settings_dirty = true;
        self.settings.tuning = self.analyzer.tuning.clone();
    }

    pub fn apply_tuning_from_ui(&mut self) {
        self.settings.tuning = self.analyzer.tuning.clone();
        self.settings_dirty = true;
    }

    pub fn apply_preset_by_name(&mut self, name: &str) {
        match load_preset(name) {
            Ok(preset) => {
                self.settings.apply_preset(&preset);
                self.analyzer.set_tuning(self.settings.tuning.clone());
                self.analyzer.beat_mut().apply_settings(&self.settings.beat);
                self.particles.set_settings(self.settings.particles.clone());
                self.preset_save_name = name.to_string();
                self.mark_settings_dirty();
                self.error_message = None;
                self.status_message = Some(format!("Loaded preset '{name}'"));
                log::info!("Loaded preset '{name}'");
            }
            Err(e) => {
                self.status_message = None;
                self.error_message = Some(format!("Preset load failed: {e}"));
            }
        }
    }

    pub fn save_current_preset(&mut self) {
        let name = sanitize_preset_name(&self.preset_save_name);
        self.preset_save_name = name.clone();
        let preset = self.settings.to_preset(&name);
        match save_preset(&preset) {
            Ok(path) => {
                self.settings.last_preset = Some(name.clone());
                self.preset_names = list_presets();
                self.mark_settings_dirty();
                self.error_message = None;
                self.status_message = Some(format!("Saved preset '{name}' → {}", path.display()));
                log::info!("Saved preset to {}", path.display());
            }
            Err(e) => {
                self.status_message = None;
                self.error_message = Some(format!("Preset save failed: {e}"));
            }
        }
    }

    pub fn refresh_presets(&mut self) {
        self.preset_names = list_presets();
    }

    pub fn pick_background(&mut self, ctx: &egui::Context) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Images", &["png", "jpg", "jpeg", "webp", "bmp", "svg", "svgz"])
            .set_title("Select background image")
            .pick_file()
        {
            if let Err(e) = self.load_background_with_ctx(ctx, path) {
                self.error_message = Some(format!("Background load failed: {e}"));
            }
        }
    }

    pub fn pick_logo(&mut self, ctx: &egui::Context) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Images", &["png", "jpg", "jpeg", "webp", "bmp", "svg", "svgz"])
            .set_title("Add logo / artwork")
            .pick_file()
        {
            if let Err(e) = self.push_logo_with_ctx(ctx, path, true) {
                self.error_message = Some(format!("Logo load failed: {e}"));
            }
        }
    }

    pub fn pick_logos(&mut self, ctx: &egui::Context) {
        let paths = rfd::FileDialog::new()
            .add_filter("Images", &["png", "jpg", "jpeg", "webp", "bmp", "svg", "svgz"])
            .set_title("Add logos (multi-select)")
            .pick_files();
        if let Some(paths) = paths {
            let mut ok = 0;
            let mut last_err = None;
            for (i, path) in paths.into_iter().enumerate() {
                match self.push_logo_with_ctx(ctx, path, i == 0 && self.logos.is_empty()) {
                    Ok(()) => ok += 1,
                    Err(e) => last_err = Some(e),
                }
            }
            if ok > 0 {
                self.error_message = None;
                self.status_message = Some(format!("Added {ok} logo(s)"));
            }
            if let Some(e) = last_err {
                self.error_message = Some(format!("Some logos failed: {e}"));
            }
        }
    }

    pub fn load_background_with_ctx(
        &mut self,
        ctx: &egui::Context,
        path: PathBuf,
    ) -> anyhow::Result<()> {
        let tex = media::load_texture_from_path(ctx, &path, "background")?;
        self.settings.background_path = Some(path.display().to_string());
        self.background = Some(tex);
        self.mark_settings_dirty();
        self.error_message = None;
        Ok(())
    }

    /// Append a logo to the playlist. If `select`, jump to it with a glitch.
    pub fn push_logo_with_ctx(
        &mut self,
        ctx: &egui::Context,
        path: PathBuf,
        select: bool,
    ) -> anyhow::Result<()> {
        let name = format!("logo_{}", self.logos.len());
        let tex = media::load_texture_from_path(ctx, &path, &name)?;
        let path_str = path.display().to_string();
        if !self.settings.logo_paths.iter().any(|p| p == &path_str) {
            self.settings.logo_paths.push(path_str.clone());
            self.settings
                .logo_styles
                .push(LogoStyle::from_stage(&self.settings.stage));
        }
        self.settings.sync_logo_styles();
        self.settings.logo_path = Some(path_str);
        self.logos.push(tex);
        if select {
            let next = self.logos.len() - 1;
            self.begin_logo_switch(next);
        }
        self.mark_settings_dirty();
        self.error_message = None;
        Ok(())
    }

    pub fn remove_logo_at(&mut self, index: usize) {
        if index >= self.logos.len() {
            return;
        }
        self.logos.remove(index);
        if index < self.settings.logo_paths.len() {
            self.settings.logo_paths.remove(index);
        }
        if index < self.settings.logo_styles.len() {
            self.settings.logo_styles.remove(index);
        }
        self.settings.sync_logo_styles();
        if self.logos.is_empty() {
            self.logo_index = 0;
            self.logo_from_index = 0;
            self.settings.logo_path = None;
        } else {
            self.logo_index %= self.logos.len();
            self.logo_from_index = self.logo_index;
            self.settings.logo_path = self.settings.logo_paths.get(self.logo_index).cloned();
        }
        self.mark_settings_dirty();
    }

    pub fn clear_logo(&mut self) {
        self.logos.clear();
        self.logo_index = 0;
        self.logo_from_index = 0;
        self.logo_switch_glitch_t = 0.0;
        self.logo_beat_glitch_t = 0.0;
        self.settings.logo_path = None;
        self.settings.logo_paths.clear();
        self.settings.logo_styles.clear();
        self.mark_settings_dirty();
    }

    pub fn clear_background(&mut self) {
        self.background = None;
        self.settings.background_path = None;
        self.mark_settings_dirty();
    }

    pub fn select_logo(&mut self, index: usize) {
        if index < self.logos.len() && index != self.logo_index {
            self.begin_logo_switch(index);
            self.mark_settings_dirty();
        }
    }

    fn begin_logo_switch(&mut self, next: usize) {
        if self.logos.is_empty() {
            return;
        }
        let next = next % self.logos.len();
        self.logo_from_index = self.logo_index.min(self.logos.len() - 1);
        self.logo_index = next;
        let hold = self
            .settings
            .logo_style(next)
            .hold_secs
            .max(0.0);
        let hold = if hold > 0.2 {
            hold
        } else {
            self.settings.stage.logo_hold_secs.max(1.0)
        };
        self.logo_hold_left = hold;
        // Transition glitch is its own envelope — not the beat glitch.
        self.logo_switch_glitch_t =
            if self.settings.stage.logo_glitch && self.logos.len() > 1 {
                1.0
            } else {
                0.0
            };
        self.settings.logo_path = self.settings.logo_paths.get(next).cloned();
    }

    fn tick_logo_cycle(&mut self, dt: f32) {
        if self.logo_switch_glitch_t > 0.0 {
            // Slightly longer for a readable logo transition.
            self.logo_switch_glitch_t = (self.logo_switch_glitch_t - dt / 0.38).max(0.0);
        }
        if self.logo_beat_glitch_t > 0.0 {
            self.logo_beat_glitch_t = (self.logo_beat_glitch_t - dt / 0.22).max(0.0);
        }

        // Beat glitch — separate from switch transition.
        let beat = self.features.beat;
        let style = self.settings.logo_style(self.logo_index);
        let want_beat_glitch = self.settings.stage.logo_glitch_on_beat && style.glitch_on_beat;
        if want_beat_glitch
            && !self.logos.is_empty()
            && beat > 0.55
            && self.last_beat_for_glitch < 0.35
        {
            let amt = self.settings.stage.logo_beat_glitch_amount.clamp(0.2, 1.5);
            self.logo_beat_glitch_t = self.logo_beat_glitch_t.max(0.85 * amt);
        }
        self.last_beat_for_glitch = beat;

        if self.logos.len() < 2 {
            return;
        }
        self.logo_hold_left -= dt;
        if self.logo_hold_left <= 0.0 {
            let next = (self.logo_index + 1) % self.logos.len();
            self.begin_logo_switch(next);
        }
    }

    fn shutdown(&mut self) {
        if self.shutting_down {
            return;
        }
        self.shutting_down = true;
        // Release audio device BEFORE tearing down GPU / egui (fixes "can't relaunch")
        self.capture.stop();
        self.settings.tuning = self.analyzer.tuning.clone();
        self.settings.particles = self.particles.settings().clone();
        // Never panic during shutdown (macOS app delegate is non-unwinding).
        if let Err(e) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = save_settings(&self.settings);
        })) {
            log::error!("settings save panicked during shutdown: {e:?}");
        }
        log::info!("Shutdown complete — audio released");
    }

    fn update_audio(&mut self) {
        if self.shutting_down {
            return;
        }
        let t0 = Instant::now();
        if let Some(samples) = self.capture.drain_samples() {
            self.features = self.analyzer.process(&samples);
            *self.features_shared.lock() = self.features.clone();
        } else {
            self.features = self.analyzer.tick_idle();
            *self.features_shared.lock() = self.features.clone();
        }
        self.timing.audio_ms = t0.elapsed().as_secs_f32() * 1000.0;
    }

    fn update_timing(&mut self) {
        let now = Instant::now();
        let dt = self
            .last_frame
            .elapsed()
            .as_secs_f32()
            .clamp(1.0 / 240.0, 1.0 / 20.0);
        self.timing.frame_ms = dt * 1000.0;
        self.last_frame = now;
        self.frame_count += 1;
        let elapsed = self.fps_timer.elapsed().as_secs_f32();
        if elapsed >= 0.5 {
            self.timing.fps = self.frame_count as f32 / elapsed;
            self.frame_count = 0;
            self.fps_timer = now;
        }

        self.tick_logo_motion(dt);
        self.tick_logo_cycle(dt);

        if self.settings_dirty && self.save_timer.elapsed().as_secs_f32() > 1.0 {
            self.settings.tuning = self.analyzer.tuning.clone();
            self.settings.particles = self.particles.settings().clone();
            let _ = save_settings(&self.settings);
            self.settings_dirty = false;
            self.save_timer = Instant::now();
        }
    }

    fn tick_logo_motion(&mut self, dt: f32) {
        let style = self.settings.logo_style(self.logo_index);
        let speed = style.spin_speed.clamp(0.0, 4.0);
        let beat = self.features.beat;
        match style.motion {
            LogoMotion::None => {}
            LogoMotion::Spin => {
                self.logo_angle += speed * std::f32::consts::TAU * dt;
            }
            LogoMotion::BeatSpin => {
                let boost = 1.0 + beat * 4.0;
                self.logo_angle += speed * boost * std::f32::consts::TAU * dt;
            }
            LogoMotion::Wobble | LogoMotion::Pendulum => {
                self.logo_motion_phase += speed * std::f32::consts::TAU * dt;
            }
        }
        if self.logo_angle > std::f32::consts::TAU * 8.0
            || self.logo_angle < -std::f32::consts::TAU * 8.0
        {
            self.logo_angle %= std::f32::consts::TAU;
        }
    }

    pub fn logo_draw_angle(&self) -> f32 {
        let style = self.settings.logo_style(self.logo_index);
        let amount = style.motion_amount.clamp(0.0, 1.0);
        match style.motion {
            LogoMotion::None => 0.0,
            LogoMotion::Spin | LogoMotion::BeatSpin => self.logo_angle,
            LogoMotion::Wobble => self.logo_motion_phase.sin() * amount * 0.35,
            LogoMotion::Pendulum => self.logo_motion_phase.sin() * amount * 0.85,
        }
    }

    pub fn active_logo_style(&self) -> LogoStyle {
        self.settings.logo_style(self.logo_index)
    }

    pub fn tick_particles(&mut self, center: egui::Pos2, dt: f32) {
        self.particles
            .update(dt, center.x, center.y, &self.features);
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let input = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::F11),
                i.key_pressed(egui::Key::Escape),
                i.key_pressed(egui::Key::F1),
            )
        });

        if input.0 {
            self.fullscreen = !self.fullscreen;
            toggle_fullscreen(ctx, self.fullscreen);
        }
        if input.1 && self.fullscreen {
            self.fullscreen = false;
            toggle_fullscreen(ctx, false);
            self.show_settings = true;
        }
        if input.2 {
            if self.settings_popout {
                self.settings_popout = false;
                self.show_settings = true;
            } else {
                self.show_settings = !self.show_settings;
            }
        }

        self.fullscreen = is_fullscreen(ctx);
    }
}

impl eframe::App for LiveVisualizerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Only treat ROOT viewport close as app quit (not the settings popout).
        if ctx.input(|i| i.viewport().parent.is_none() && i.viewport().close_requested()) {
            self.shutdown();
            return;
        }

        if self.shutting_down {
            return;
        }

        self.handle_shortcuts(ctx);
        self.update_audio();
        self.update_timing();
        ctx.request_repaint();

        // Detached settings window — drag to the DJ laptop screen while
        // the main viewport stays fullscreen on the TV.
        if self.settings_popout {
            let mut open = true;
            ctx.show_viewport_immediate(
                egui::ViewportId::from_hash_of("live_visualizer_settings"),
                egui::ViewportBuilder::default()
                    .with_title("Iris Visualizer — Controls")
                    .with_inner_size([400.0, 780.0])
                    .with_min_inner_size([320.0, 420.0])
                    .with_resizable(true),
                |ctx, _class| {
                    ctx.request_repaint();
                    if ctx.input(|i| i.viewport().close_requested()) {
                        open = false;
                    }
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui::draw_settings_contents(ui, self);
                    });
                },
            );
            if !open {
                self.settings_popout = false;
                self.show_settings = true;
            }
        }

        // Main stage: fullscreen DJ mode when settings are popped out or hidden
        let hide_docked_ui = self.settings_popout || !self.show_settings;
        if self.fullscreen && hide_docked_ui {
            egui::CentralPanel::default()
                .frame(egui::Frame::none().fill(egui::Color32::BLACK))
                .show(ctx, |ui| {
                    ui::draw_stage(ui, self, false);
                });
            return;
        }

        if self.show_settings && !self.settings_popout {
            ui::draw_settings_panel(ctx, self);
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(egui::Color32::from_rgb(8, 8, 12)))
            .show(ctx, |ui| {
                ui::draw_stage(ui, self, self.show_debug && !self.fullscreen);
            });
    }
}

impl Drop for LiveVisualizerApp {
    fn drop(&mut self) {
        // Safety net if the window is killed without a close event.
        self.shutdown();
    }
}
