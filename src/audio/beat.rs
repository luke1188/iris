//! Kick detector: time-domain attack + kick-body flux.
//!
//! Sustained bass / 808 rumble has energy but almost no derivative.
//! A kick has a sharp attack (~5–20 ms) *and* a 50–100 Hz body jump.
//! We never trigger on bass level alone.

use serde::{Deserialize, Serialize};

/// Which signal(s) the detector listens to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BeatMode {
    /// Kick attack + kick-body flux (default, tracks drums not basslines).
    #[default]
    BassKick,
    /// Follows bass level (bounces with sustained bass too).
    BassLevel,
    /// Strictest: strong attack required, ignores rumble.
    KickOnly,
    /// Kick path plus a little overall-energy onset.
    FullMix,
}

#[derive(Debug, Clone)]
pub struct BeatDetector {
    pub sensitivity: f32,
    pub kick_sensitivity: f32,
    pub cooldown: f32,
    pub mode: BeatMode,
    pub bass_weight: f32,
    pub rms_weight: f32,
    /// Slow floor of kick-body energy (rumble / sustain).
    body_floor: f32,
    cooldown_left: f32,
    pulse: f32,
    last_body: f32,
    last_click: f32,
    last_attack: f32,
    last_rms: f32,
    /// After a hit, wait until the attack dies before allowing another.
    rearm: bool,
    /// Attack and body can land on adjacent frames (FFT lag).
    attack_hold: f32,
    body_hold: f32,
}

impl Default for BeatDetector {
    fn default() -> Self {
        Self {
            sensitivity: 0.85,
            kick_sensitivity: 1.0,
            cooldown: 0.22,
            mode: BeatMode::KickOnly,
            bass_weight: 1.0,
            rms_weight: 0.0,
            body_floor: 0.04,
            cooldown_left: 0.0,
            pulse: 0.0,
            last_body: 0.0,
            last_click: 0.0,
            last_attack: 0.0,
            last_rms: 0.0,
            rearm: false,
            attack_hold: 0.0,
            body_hold: 0.0,
        }
    }
}

impl BeatDetector {
    pub fn apply_settings(&mut self, s: &crate::config::BeatSettings) {
        self.sensitivity = s.sensitivity;
        self.kick_sensitivity = s.kick_sensitivity;
        self.cooldown = s.cooldown;
        self.mode = s.mode;
        self.bass_weight = s.bass_weight;
        self.rms_weight = s.rms_weight;
    }

    /// `body` = raw 55–95 Hz. `click` = raw 2–5 kHz. `attack` = time-domain transient.
    /// `bass` / `rms` are display envelopes (BassLevel / FullMix only).
    pub fn process(&mut self, bass: f32, rms: f32, body: f32, click: f32, attack: f32) -> f32 {
        const DT: f32 = 1.0 / 60.0;
        self.cooldown_left = (self.cooldown_left - DT).max(0.0);
        self.pulse *= 0.76;

        let body = body.max(0.0);
        let click = click.max(0.0);
        let attack = attack.max(0.0);

        // Very slow floor — rumble should become the baseline, not a trigger.
        if body > self.body_floor {
            self.body_floor = self.body_floor * 0.996 + body * 0.004;
        } else {
            self.body_floor = self.body_floor * 0.97 + body * 0.03;
        }

        let body_flux = (body - self.last_body).max(0.0);
        let attack_flux = (attack - self.last_attack).max(0.0);
        let rms_flux = (rms - self.last_rms).max(0.0);
        self.last_body = body;
        self.last_click = click;
        self.last_attack = attack;
        self.last_rms = rms;

        // Rearm after the hit: attack must fall so one kick ≠ a burst of triggers.
        if self.rearm {
            if attack < 0.08 && body_flux < 0.012 {
                self.rearm = false;
            }
            return self.pulse.clamp(0.0, 1.0);
        }

        if self.cooldown_left > 0.0 {
            return self.pulse.clamp(0.0, 1.0);
        }

        let sens = self.sensitivity.clamp(0.25, 3.5);
        let punch = self.kick_sensitivity.clamp(0.25, 3.0);
        // Strict at 1.0 — bass notes should not clear this.
        let attack_need = (0.28 / (sens * 0.45 + punch * 0.55)).clamp(0.14, 0.5);
        let flux_need = (0.10 / sens).clamp(0.055, 0.22);
        let body_min = (0.10 / sens).clamp(0.055, 0.2);

        // Snare / hat: lots of click, little kick body.
        let looks_like_snare = click > body * 2.2 && body < 0.22;

        let sharp = attack > attack_need && attack_flux > attack_need * 0.22;
        let body_hit = body_flux > flux_need && body > self.body_floor * 1.35 + body_min;

        if sharp {
            self.attack_hold = 1.0;
        } else {
            self.attack_hold *= 0.48;
        }
        if body_hit {
            self.body_hold = 1.0;
        } else {
            self.body_hold *= 0.48;
        }

        let kick_onset =
            self.attack_hold > 0.55 && self.body_hold > 0.55 && !looks_like_snare;

        let hit = match self.mode {
            BeatMode::KickOnly | BeatMode::BassKick => kick_onset,
            BeatMode::BassLevel => {
                kick_onset
                    || (self.bass_weight > 0.2
                        && bass > 0.55
                        && attack > attack_need * 0.55
                        && body_flux > flux_need * 0.45)
            }
            BeatMode::FullMix => {
                kick_onset
                    || (self.rms_weight > 0.15
                        && rms_flux > 0.08
                        && attack > attack_need * 0.5)
            }
        };

        if hit {
            self.pulse = 1.0;
            self.cooldown_left = self.cooldown.max(0.18);
            self.rearm = true;
            self.attack_hold = 0.0;
            self.body_hold = 0.0;
        }

        self.pulse.clamp(0.0, 1.0)
    }
}
