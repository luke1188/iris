//! Lightweight particle system (CPU simulate, GPU/egui draw).
//! Target ~500–2000 particles with audio-reactive motion.

use crate::audio::AudioFeatures;
use crate::config::ParticleSettings;
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

    pub fn draw(&self, painter: &egui::Painter, color_mode: crate::config::ColorMode) {
        if !self.settings.enabled {
            return;
        }
        let opacity = self.settings.opacity.clamp(0.05, 1.0) * 0.85;
        let glow = self.settings.glow.clamp(0.0, 1.0) * 0.45;

        for p in &self.particles {
            let t = (p.life / p.max_life).clamp(0.0, 1.0);
            let a = (opacity * t * 255.0) as u8;
            if a < 8 {
                continue;
            }
            let color = particle_color(p.hue, t, a, color_mode);
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

fn particle_color(hue: f32, life_t: f32, alpha: u8, mode: crate::config::ColorMode) -> Color32 {
    match mode {
        crate::config::ColorMode::Rgb => {
            let (r, g, b) = hsv(hue * 360.0, 0.9, 0.55 + 0.45 * life_t);
            Color32::from_rgba_unmultiplied(
                (r * 255.0) as u8,
                (g * 255.0) as u8,
                (b * 255.0) as u8,
                alpha,
            )
        }
        crate::config::ColorMode::Mono => {
            let v = (180.0 + 75.0 * life_t) as u8;
            Color32::from_rgba_unmultiplied(v, v, v, alpha)
        }
        crate::config::ColorMode::Cyan => {
            Color32::from_rgba_unmultiplied(40, 220, 255, alpha)
        }
        crate::config::ColorMode::Amber => {
            Color32::from_rgba_unmultiplied(255, 170, 60, alpha)
        }
        crate::config::ColorMode::Magenta => {
            Color32::from_rgba_unmultiplied(255, 70, 180, alpha)
        }
    }
}

fn hsv(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    (r + m, g + m, b + m)
}
