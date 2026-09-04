//! Lightweight particle system (CPU simulate, GPU/egui draw).
//! Target ~500–2000 particles with audio-reactive motion.

use crate::audio::AudioFeatures;
use crate::config::{neon_rainbow, ColorMode, ParticleSettings, RgbColor};
use egui::{Color32, Pos2, Shape};

#[derive(Clone, Copy)]
struct Particle {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
    max_life: f32,
    size: f32,
    hue: f32,
}

pub struct ParticleSystem {
    particles: Vec<Particle>,
    settings: ParticleSettings,
    spawn_accum: f32,
    rng_state: u32,
    time: f32,
}

impl Default for ParticleSystem {
    fn default() -> Self {
        Self::new(ParticleSettings::default())
    }
}

impl ParticleSystem {
    pub fn new(settings: ParticleSettings) -> Self {
        let cap = settings.count.max(1) as usize;
        Self {
            particles: Vec::with_capacity(cap),
            settings,
            spawn_accum: 0.0,
            rng_state: 0xA341_316C,
            time: 0.0,
        }
    }

    pub fn set_settings(&mut self, settings: ParticleSettings) {
        let cap = settings.count.max(1) as usize;
        if self.particles.capacity() < cap {
            self.particles.reserve(cap - self.particles.capacity());
        }
        if self.particles.len() > cap {
            self.particles.truncate(cap);
        }
        self.settings = settings;
    }

    pub fn settings(&self) -> &ParticleSettings {
        &self.settings
    }

    pub fn settings_mut(&mut self) -> &mut ParticleSettings {
        &mut self.settings
    }

    fn rand(&mut self) -> f32 {
        // xorshift32
        let mut x = self.rng_state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng_state = x.max(1);
        (self.rng_state as f32) / (u32::MAX as f32)
    }

    fn spawn_one(&mut self, cx: f32, cy: f32, burst: bool, features: &AudioFeatures) {
        if self.particles.len() >= self.settings.count.max(1) as usize {
            return;
        }
        let angle = self.rand() * std::f32::consts::TAU;
        let (s, c) = angle.sin_cos();
        let spread = self.settings.spread.clamp(0.1, 2.0);
        let r = if burst {
            8.0 + self.rand() * 40.0 * spread
        } else {
            30.0 + self.rand() * 180.0 * spread
        };
        let speed = self.settings.speed * (0.4 + self.rand());
        let outward = if burst {
            speed * (1.2 + features.beat * self.settings.beat_reaction)
        } else {
            speed * 0.35
        };
        let life = self.settings.lifetime * (0.5 + self.rand());
        let jx = (self.rand() - 0.5) * 20.0;
        let jy = (self.rand() - 0.5) * 20.0;
        let size = self.settings.size * (0.6 + self.rand() * 0.8);
        let hue = self.rand();
        self.particles.push(Particle {
            x: cx + c * r,
            y: cy + s * r,
            vx: c * outward + jx,
            vy: s * outward + jy,
            life,
            max_life: life,
            size,
            hue,
        });
    }

    pub fn update(&mut self, dt: f32, cx: f32, cy: f32, features: &AudioFeatures) {
        if !self.settings.enabled {
            self.particles.clear();
            return;
        }

        self.time += dt;

        let bass_push = features.kick * self.settings.bass_reaction * 90.0;
        let high_spark = features.high * 0.8;

        // Continuous ambient spawn
        self.spawn_accum += dt * (8.0 + features.rms * 40.0 + high_spark * 30.0);
        while self.spawn_accum >= 1.0 {
            self.spawn_accum -= 1.0;
            self.spawn_one(cx, cy, false, features);
        }

        // Beat burst
        if features.beat > 0.55 {
            let n = (6.0 + features.beat * 18.0 * self.settings.beat_reaction) as i32;
            for _ in 0..n {
                self.spawn_one(cx, cy, true, features);
            }
        }

        let gravity = self.settings.gravity;
        let drag = 0.985_f32;

        self.particles.retain_mut(|p| {
            let dx = p.x - cx;
            let dy = p.y - cy;
            let dist = (dx * dx + dy * dy).sqrt().max(1.0);
            let nx = dx / dist;
            let ny = dy / dist;

            // Bass pushes outward from center
            p.vx += nx * bass_push * dt;
            p.vy += ny * bass_push * dt;
            p.vy += gravity * 60.0 * dt;
            p.vx *= drag;
            p.vy *= drag;
            // Cap speed so particles stay soft dots, not streaky lasers.
            let speed = (p.vx * p.vx + p.vy * p.vy).sqrt();
            let max_speed = 220.0;
            if speed > max_speed {
                let s = max_speed / speed;
                p.vx *= s;
                p.vy *= s;
            }
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            p.life -= dt;
            p.life > 0.0
        });
    }

    pub fn draw(&self, painter: &egui::Painter) {
        if !self.settings.enabled {
            return;
        }
        let opacity = self.settings.opacity.clamp(0.05, 1.0) * 0.85;
        let glow = self.settings.glow.clamp(0.0, 1.0) * 0.45;
        let cycle = self.time * self.settings.cycle_speed.clamp(0.0, 3.0);

        for p in &self.particles {
            let t = (p.life / p.max_life).clamp(0.0, 1.0);
            let a = (opacity * t * 255.0) as u8;
            if a < 8 {
                continue;
            }
            let color = particle_color(&self.settings, p.hue, t, a, cycle);
            let pos = Pos2::new(p.x, p.y);
            let r = (p.size * (0.45 + 0.4 * t)).clamp(0.6, 4.5);

            // Soft dots only — no trail lines (those read as laser streaks).
            if glow > 0.05 {
                let ga = ((a as f32) * glow * 0.25) as u8;
                painter.circle_filled(
                    pos,
                    r * 1.8,
                    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), ga),
                );
            }
            painter.add(Shape::circle_filled(pos, r, color));
        }
    }
}

fn particle_color(
    settings: &ParticleSettings,
    hue: f32,
    life_t: f32,
    alpha: u8,
    cycle: f32,
) -> Color32 {
    let boost = 0.7 + 0.3 * life_t;
    let rgb = match settings.color_mode {
        ColorMode::Solid | ColorMode::Cyan | ColorMode::Amber | ColorMode::Magenta | ColorMode::Mono => {
            let c = match settings.color_mode {
                ColorMode::Cyan => RgbColor::new(0.2, 0.82, 1.0),
                ColorMode::Amber => RgbColor::new(1.0, 0.67, 0.22),
                ColorMode::Magenta => RgbColor::new(1.0, 0.22, 0.75),
                ColorMode::Mono => {
                    let g = 0.7;
                    RgbColor::new(g, g, g)
                }
                _ => settings.solid,
            };
            c.scale(boost)
        }
        ColorMode::Gradient | ColorMode::Bands => settings
            .gradient_low
            .lerp(settings.gradient_high, hue)
            .scale(boost),
        ColorMode::Rainbow => {
            let (r, g, b) = neon_rainbow(hue);
            RgbColor::new(r, g, b).scale(boost)
        }
        ColorMode::Cycle => {
            let (r, g, b) = neon_rainbow((hue + cycle).fract());
            RgbColor::new(r, g, b).scale(boost)
        }
    };
    Color32::from_rgba_unmultiplied(
        (rgb.r * 255.0) as u8,
        (rgb.g * 255.0) as u8,
        (rgb.b * 255.0) as u8,
        alpha,
    )
}
