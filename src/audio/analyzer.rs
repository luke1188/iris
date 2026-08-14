//! Real-time audio features produced by the analyzer.
//!
//! The renderer consumes `AudioFeatures` and must not talk to the audio device.

use serde::{Deserialize, Serialize};

/// Lightweight audio analysis snapshot for one frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioFeatures {
    pub rms: f32,
    pub bass: f32,
    pub low_mid: f32,
    pub mid: f32,
    pub high_mid: f32,
    pub high: f32,
    /// 0..1 pulse when a kick/beat is detected.
    pub beat: f32,
    /// Log-spaced spectrum magnitudes, typically 128–256 bins, normalized 0..1.
    pub spectrum: Vec<f32>,
}

impl Default for AudioFeatures {
    fn default() -> Self {
        Self {
            rms: 0.0,
            bass: 0.0,
            low_mid: 0.0,
            mid: 0.0,
            high_mid: 0.0,
            high: 0.0,
            beat: 0.0,
            spectrum: vec![0.0; 256],
        }
    }
}

/// Per-band tuning for loud venues — keep bass from pinning while mids/highs stay alive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandTuning {
    /// Master input gain applied before band analysis.
    pub input_gain: f32,
    /// Energy below this (after gain) is treated as silence / reduced.
    pub threshold: f32,
    pub bass_gain: f32,
    pub low_mid_gain: f32,
    pub mid_gain: f32,
    pub high_mid_gain: f32,
    pub high_gain: f32,
    /// Extra scale for the low end of the log spectrum ring.
    pub spectrum_bass_tilt: f32,
    /// Soft-clip knee: lower = more headroom before hitting 1.0.
    pub ceiling: f32,
    pub attack: f32,
    pub release: f32,
    pub sensitivity: f32,
}

impl Default for BandTuning {
    fn default() -> Self {
        // Defaults biased for a loud bar: bass pulled back, highs a bit up.
        Self {
            input_gain: 0.85,
            threshold: 0.04,
            bass_gain: 0.35,
            low_mid_gain: 0.75,
            mid_gain: 1.0,
            high_mid_gain: 1.15,
            high_gain: 1.25,
            spectrum_bass_tilt: 0.45,
            ceiling: 0.85,
            attack: 0.32,
            release: 0.14,
            sensitivity: 1.2,
        }
    }
}

impl BandTuning {
    pub fn reset_defaults(&mut self) {
        *self = Self::default();
    }
}

/// Band-energy / spectrum analyzer with attack/release smoothing.
pub struct Analyzer {
    sample_rate: u32,
    fft_size: usize,
    fft: crate::audio::fft::FftProcessor,
    beat: crate::audio::beat::BeatDetector,
    window_buf: Vec<f32>,
    hann: Vec<f32>,
    smooth_spectrum: Vec<f32>,
    mag_scratch: Vec<f32>,
    spectrum_targets: Vec<f32>,
    /// rms, bass, low_mid, mid, high_mid, high
    smooth_bands: [f32; 6],
    pub tuning: BandTuning,
}

impl Analyzer {
    pub fn new(sample_rate: u32) -> Self {
        let fft_size = 4096; // finer frequency resolution for peakier ring
        let spectrum_bins = 256;
        let mut hann = vec![0.0_f32; fft_size];
        for (i, h) in hann.iter_mut().enumerate() {
            *h = 0.5
                * (1.0
                    - (2.0 * std::f32::consts::PI * i as f32 / (fft_size as f32 - 1.0)).cos());
        }
        Self {
            sample_rate,
            fft_size,
            fft: crate::audio::fft::FftProcessor::new(fft_size),
            beat: crate::audio::beat::BeatDetector::default(),
            window_buf: vec![0.0; fft_size],
            hann,
            smooth_spectrum: vec![0.0; spectrum_bins],
            mag_scratch: vec![0.0; fft_size / 2],
            spectrum_targets: vec![0.0; spectrum_bins],
            smooth_bands: [0.0; 6],
            tuning: BandTuning::default(),
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate.max(1);
    }

    pub fn set_tuning(&mut self, tuning: BandTuning) {
        self.tuning = tuning;
    }

    pub fn beat_mut(&mut self) -> &mut crate::audio::beat::BeatDetector {
        &mut self.beat
    }

    /// Process a mono (or already-downmixed) sample chunk. Uses the latest `fft_size` samples.
    pub fn process(&mut self, samples: &[f32]) -> AudioFeatures {
        if samples.is_empty() {
            return self.tick_idle();
        }

        let gain = self.tuning.input_gain.clamp(0.0, 4.0);
        let sens = self.tuning.sensitivity.clamp(0.05, 8.0);
        let attack = self.tuning.attack.clamp(0.01, 1.0);
        let release = self.tuning.release.clamp(0.01, 1.0);
        let threshold = self.tuning.threshold.clamp(0.0, 0.5);
        let ceiling = self.tuning.ceiling.clamp(0.2, 1.5);

        let n = samples.len().min(self.fft_size);
        let start = self.fft_size - n;
        self.window_buf[..start].fill(0.0);
        for (dst, &src) in self.window_buf[start..]
            .iter_mut()
            .zip(&samples[samples.len() - n..])
        {
            *dst = src * gain;
        }

        for i in 0..self.fft_size {
            self.window_buf[i] *= self.hann[i];
        }

        self.fft
            .compute_magnitudes(&self.window_buf, &mut self.mag_scratch);

        let sr = self.sample_rate as f32;
        let bin_hz = sr / self.fft_size as f32;
        let nyquist_bins = self.mag_scratch.len();

        let mut sum_sq = 0.0_f32;
        for &s in &self.window_buf {
            sum_sq += s * s;
        }
        let rms_raw = (sum_sq / self.fft_size as f32).sqrt() * sens * 3.5;

        let band_raw = [
            gate(rms_raw, threshold),
            gate(
                band_energy(&self.mag_scratch, bin_hz, 20.0, 150.0, nyquist_bins)
                    * sens
                    * self.tuning.bass_gain,
                threshold,
            ),
            gate(
                band_energy(&self.mag_scratch, bin_hz, 150.0, 400.0, nyquist_bins)
                    * sens
                    * self.tuning.low_mid_gain,
                threshold,
            ),
            gate(
                band_energy(&self.mag_scratch, bin_hz, 400.0, 2000.0, nyquist_bins)
                    * sens
                    * self.tuning.mid_gain,
                threshold,
            ),
            gate(
                band_energy(&self.mag_scratch, bin_hz, 2000.0, 6000.0, nyquist_bins)
                    * sens
                    * self.tuning.high_mid_gain,
                threshold,
            ),
            gate(
                band_energy(&self.mag_scratch, bin_hz, 6000.0, 16_000.0, nyquist_bins)
                    * sens
                    * self.tuning.high_gain,
                threshold,
            ),
        ];

        for i in 0..6 {
            let shaped = soft_ceiling(band_raw[i], ceiling);
            self.smooth_bands[i] = smooth(self.smooth_bands[i], shaped, attack, release);
        }

        // Spectrum wants snappier attack than the meters so peaks punch.
        let spec_attack = (attack * 1.55).clamp(0.08, 1.0);
        let spec_release = (release * 0.85).clamp(0.04, 0.8);
        remap_log_spectrum(
            &self.mag_scratch,
            bin_hz,
            &mut self.smooth_spectrum,
            &mut self.spectrum_targets,
            &self.tuning,
            sens,
            spec_attack,
            spec_release,
            threshold,
            ceiling,
        );

        let bass_s = self.smooth_bands[1].clamp(0.0, 1.0);
        let rms_s = self.smooth_bands[0].clamp(0.0, 1.0);
        let beat = self.beat.process(bass_s, rms_s);

        AudioFeatures {
            rms: rms_s,
            bass: bass_s,
            low_mid: self.smooth_bands[2].clamp(0.0, 1.0),
            mid: self.smooth_bands[3].clamp(0.0, 1.0),
            high_mid: self.smooth_bands[4].clamp(0.0, 1.0),
            high: self.smooth_bands[5].clamp(0.0, 1.0),
            beat,
            spectrum: self.smooth_spectrum.clone(),
        }
    }

    pub fn tick_idle(&mut self) -> AudioFeatures {
        let release = self.tuning.release.clamp(0.01, 1.0);
        for b in &mut self.smooth_bands {
            *b *= 1.0 - release * 0.5;
        }
        for s in &mut self.smooth_spectrum {
            *s *= 1.0 - release * 0.5;
        }
        let beat = self.beat.process(
            self.smooth_bands[1].clamp(0.0, 1.0),
            self.smooth_bands[0].clamp(0.0, 1.0),
        );
        AudioFeatures {
            rms: self.smooth_bands[0].clamp(0.0, 1.0),
            bass: self.smooth_bands[1].clamp(0.0, 1.0),
            low_mid: self.smooth_bands[2].clamp(0.0, 1.0),
            mid: self.smooth_bands[3].clamp(0.0, 1.0),
            high_mid: self.smooth_bands[4].clamp(0.0, 1.0),
            high: self.smooth_bands[5].clamp(0.0, 1.0),
            beat,
            spectrum: self.smooth_spectrum.clone(),
        }
    }
}

fn gate(value: f32, threshold: f32) -> f32 {
    if value <= threshold {
        return 0.0;
    }
    // Soft expand above threshold so quiet content isn't crushed abruptly
    let span = (1.0 - threshold).max(0.05);
    ((value - threshold) / span).clamp(0.0, 2.0)
}

fn soft_ceiling(value: f32, ceiling: f32) -> f32 {
    if value <= ceiling {
        return value.clamp(0.0, 1.5);
    }
    // Gentle rolloff past the knee instead of hard clip at 1.0
    let over = value - ceiling;
    let soft = ceiling + over / (1.0 + over * 3.0);
    soft.clamp(0.0, 1.0)
}

fn smooth(prev: f32, next: f32, attack: f32, release: f32) -> f32 {
    if next > prev {
        prev + (next - prev) * attack
    } else {
        prev + (next - prev) * release
    }
}

fn band_energy(mags: &[f32], bin_hz: f32, f_lo: f32, f_hi: f32, nyquist_bins: usize) -> f32 {
    let i0 = ((f_lo / bin_hz) as usize).clamp(1, nyquist_bins.saturating_sub(1));
    let i1 = ((f_hi / bin_hz) as usize).clamp(i0 + 1, nyquist_bins);
    let mut sum = 0.0_f32;
    let mut count = 0_u32;
    for mag in &mags[i0..i1] {
        sum += mag * mag;
        count += 1;
    }
    if count == 0 {
        return 0.0;
    }
    let rms = (sum / count as f32).sqrt();
    (rms * 10.0).clamp(0.0, 2.0)
}

/// Peak magnitude in a frequency range — preserves distinct frequency spikes.
fn band_peak(mags: &[f32], bin_hz: f32, f_lo: f32, f_hi: f32, nyquist_bins: usize) -> f32 {
    let i0 = ((f_lo / bin_hz) as usize).clamp(1, nyquist_bins.saturating_sub(1));
    let i1 = ((f_hi / bin_hz) as usize).clamp(i0 + 1, nyquist_bins);
    let mut peak = 0.0_f32;
    for mag in &mags[i0..i1] {
        peak = peak.max(*mag);
    }
    // Mild compression so quiet harmonics still register without crushing loud peaks
    (peak * 14.0).powf(0.85).clamp(0.0, 2.0)
}

fn remap_log_spectrum(
    mags: &[f32],
    bin_hz: f32,
    out: &mut [f32],
    targets: &mut [f32],
    tuning: &BandTuning,
    sensitivity: f32,
    attack: f32,
    release: f32,
    threshold: f32,
    ceiling: f32,
) {
    let n = out.len().min(targets.len());
    let f_min = 30.0_f32;
    let f_max = (bin_hz * (mags.len() as f32 - 1.0)).min(14_000.0);
    let log_min = f_min.ln();
    let log_max = f_max.ln();

    let mut peak = 0.001_f32;

    for i in 0..n {
        let t0 = i as f32 / n as f32;
        let t1 = (i + 1) as f32 / n as f32;
        let f0 = (log_min + (log_max - log_min) * t0).exp();
        let f1 = (log_min + (log_max - log_min) * t1).exp();
        // Prefer geometric center of the log bin
        let f_center = (f0 * f1).sqrt();
        let band_gain = spectrum_band_gain(f_center, tuning) * perceptual_weight(f_center);

        // Mix peak (frequency detail) with a touch of energy (body)
        let peak_e = band_peak(mags, bin_hz, f0, f1, mags.len());
        let body_e = band_energy(mags, bin_hz, f0, f1, mags.len());
        let energy = peak_e * 0.78 + body_e * 0.22;

        let gated = gate(energy * sensitivity * band_gain, threshold * 0.65);
        // Emphasize peaks relative to a soft curve (Trap Nation punch)
        let shaped = gated.powf(0.78);
        targets[i] = soft_ceiling(shaped, ceiling);
        peak = peak.max(targets[i]);
    }

    // Local contrast: boost bins that stick out from neighbors (real frequency peaks)
    for i in 0..n {
        let left = targets[(i + n - 1) % n];
        let right = targets[(i + 1) % n];
        let local = (left + right) * 0.5;
        let delta = (targets[i] - local).max(0.0);
        targets[i] = (targets[i] + delta * 0.55).clamp(0.0, 1.35);
        peak = peak.max(targets[i]);
    }

    let norm = if peak > 0.4 {
        (0.92 / peak).clamp(0.55, 1.2)
    } else {
        1.0
    };

    for i in 0..n {
        let target = (targets[i] * norm).clamp(0.0, 1.0);
        out[i] = smooth(out[i], target, attack, release);
    }

    // Very light blur only — keep peaks sharp enough to read as frequencies
    circular_blur(out, targets, n, 1);
}

fn circular_blur(data: &mut [f32], scratch: &mut [f32], n: usize, radius: usize) {
    if n == 0 || radius == 0 {
        return;
    }
    scratch[..n].copy_from_slice(&data[..n]);
    let denom = (radius * 2 + 1) as f32;
    for i in 0..n {
        let mut sum = 0.0_f32;
        for d in 0..=(radius * 2) {
            let j = (i + n + d - radius) % n;
            sum += scratch[j];
        }
        // Preserve peaks: take max of blur and 92% of original
        let blurred = sum / denom;
        data[i] = blurred.max(scratch[i] * 0.92);
    }
}

/// Mild mid tame — enough to stop pinning, not enough to erase frequency shape.
fn perceptual_weight(freq_hz: f32) -> f32 {
    if freq_hz < 120.0 {
        1.1
    } else if freq_hz < 400.0 {
        0.9
    } else if freq_hz < 1_500.0 {
        0.68
    } else if freq_hz < 4_000.0 {
        0.62
    } else if freq_hz < 9_000.0 {
        0.85
    } else {
        1.0
    }
}

fn spectrum_band_gain(freq_hz: f32, tuning: &BandTuning) -> f32 {
    if freq_hz < 150.0 {
        tuning.bass_gain * tuning.spectrum_bass_tilt
    } else if freq_hz < 400.0 {
        tuning.low_mid_gain
    } else if freq_hz < 2000.0 {
        tuning.mid_gain
    } else if freq_hz < 6000.0 {
        tuning.high_mid_gain
    } else {
        tuning.high_gain
    }
}
