//! Lightweight kick/beat detector from bass energy transients.

use serde::{Deserialize, Serialize};

/// Which signal(s) the detector listens to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BeatMode {
    /// Bass transients only — classic kick / 808 hits.
    #[default]
    BassKick,
    /// Follows bass level (bounces with sustained bass too).
    BassLevel,
    /// Stricter kick/transient only (ignores sustained bass).
    KickOnly,
    /// Bass + overall energy (most sensitive / busy).
    FullMix,
}

#[derive(Debug, Clone)]
pub struct BeatDetector {
    pub sensitivity: f32,
    pub kick_sensitivity: f32,
    pub cooldown: f32,
    pub mode: BeatMode,
    /// How much bass contributes (0..2).
    pub bass_weight: f32,
    /// How much overall RMS contributes (0..2).
    pub rms_weight: f32,
    envelope: f32,
    cooldown_left: f32,
    pulse: f32,
    last_bass: f32,
    last_rms: f32,
}

impl Default for BeatDetector {
    fn default() -> Self {
        Self {
            sensitivity: 1.85,
            kick_sensitivity: 1.45,
            cooldown: 0.09,
            mode: BeatMode::BassKick,
            bass_weight: 1.0,
            rms_weight: 0.35,
            envelope: 0.0,
            cooldown_left: 0.0,
            pulse: 0.0,
            last_bass: 0.0,
            last_rms: 0.0,
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

    /// Returns a decaying 0..1 beat pulse.
    pub fn process(&mut self, bass: f32, rms: f32) -> f32 {
        const DT: f32 = 1.0 / 60.0;
        self.cooldown_left = (self.cooldown_left - DT).max(0.0);
        self.pulse *= 0.88;

        let bass_w = bass * self.bass_weight.clamp(0.0, 2.5);
        let rms_w = rms * self.rms_weight.clamp(0.0, 2.5);

        self.envelope = self.envelope * 0.94 + bass_w * 0.06;
        let sens = self.sensitivity.max(0.15);
        let threshold = (self.envelope * 1.05 + 0.035) / sens;

        let bass_delta = (bass - self.last_bass).max(0.0);
        let rms_delta = (rms - self.last_rms).max(0.0);
        self.last_bass = bass;
        self.last_rms = rms;

        let kick = (bass_delta * self.bass_weight.max(0.1) * 1.35
            + rms_delta * self.rms_weight * 0.55)
            * self.kick_sensitivity;

        let hit = match self.mode {
            BeatMode::BassKick => {
                // Clear bass transient above adaptive floor
                bass_w > threshold && kick > (0.022 / sens.max(0.5))
            }
            BeatMode::BassLevel => {
                // Transient OR strong sustained bass
                (bass_w > threshold && kick > 0.015)
                    || (bass_w > (0.28 / sens).clamp(0.12, 0.55) && kick > 0.008)
            }
            BeatMode::KickOnly => {
                // Strict transient — ignores rumble
                bass_delta * self.kick_sensitivity > (0.045 / sens)
                    && bass_w > threshold * 0.7
            }
            BeatMode::FullMix => {
                (bass_w > threshold && kick > 0.018)
                    || (bass > 0.22 && kick > 0.028)
                    || (rms_w > 0.15 && rms_delta > 0.035)
            }
        };

        if self.cooldown_left <= 0.0 && hit {
            self.pulse = 1.0;
            self.cooldown_left = self.cooldown;
        }

        self.pulse.clamp(0.0, 1.0)
    }
}
