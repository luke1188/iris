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
    /// Fast kick-band envelope (45–110 Hz) — used for disc bounce.
    #[serde(default)]
    pub kick: f32,
    /// 20–60 Hz sub energy from the shared spectrum processor (0..1).
    #[serde(default)]
    pub sub: f32,
    /// Broadband energy from the shared processor (0..1).
    #[serde(default)]
    pub overall_energy: f32,
    /// Onset / punch energy from the shared processor (0..1).
    #[serde(default)]
    pub transient_energy: f32,
    /// Log-spaced spectrum magnitudes, typically 128–256 bins, normalized 0..1.
    pub spectrum: Vec<f32>,
    /// Pre-smoothing bands (same length as `spectrum`).
    #[serde(default)]
    pub spectrum_raw: Vec<f32>,
    /// Decaying per-band peaks for optional LED caps.
    #[serde(default)]
    pub spectrum_peaks: Vec<f32>,
    /// Recent time-domain samples for the center oscilloscope (−1..1).
    #[serde(default)]
    pub waveform: Vec<f32>,
    /// Shared processor output for every visual system (spectrum, lighting, particles).
    #[serde(default)]
    pub processed: crate::audio::ProcessedSpectrum,
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
            kick: 0.0,
            sub: 0.0,
            overall_energy: 0.0,
            transient_energy: 0.0,
            spectrum: vec![0.0; 224],
            spectrum_raw: vec![0.0; 224],
            spectrum_peaks: vec![0.0; 224],
            waveform: vec![0.0; 256],
            processed: crate::audio::ProcessedSpectrum::default(),
        }
    }
}

impl AudioFeatures {
    /// Shared 0..1 spectrum + band energies for every visual system.
    pub fn processed(&self) -> &crate::audio::ProcessedSpectrum {
        &self.processed
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
            input_gain: 0.80,
            threshold: 0.05,
            bass_gain: 0.42,
            low_mid_gain: 0.82,
            mid_gain: 1.05,
            high_mid_gain: 1.18,
            high_gain: 1.22,
            spectrum_bass_tilt: 0.62,
            ceiling: 0.82,
            attack: 0.34,
            release: 0.13,
            sensitivity: 1.15,
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
    mag_scratch: Vec<f32>,
    spectrum: crate::audio::spectrum::SpectrumProcessor,
    /// rms, bass, low_mid, mid, high_mid, high
    smooth_bands: [f32; 6],
    /// Slow adaptive floor so constant rumble doesn't pin the meters.
    band_floor: [f32; 6],
    kick_floor: f32,
    smooth_kick: f32,
    /// Downsampled recent waveform for the center scope.
    waveform: Vec<f32>,
    pub tuning: BandTuning,
}

impl Analyzer {
    pub fn new(sample_rate: u32) -> Self {
        let fft_size = 4096; // finer frequency resolution for peakier ring
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
            mag_scratch: vec![0.0; fft_size / 2],
            spectrum: crate::audio::SpectrumProcessor::new(224),
            smooth_bands: [0.0; 6],
            band_floor: [0.02; 6],
            kick_floor: 0.02,
            smooth_kick: 0.0,
            waveform: vec![0.0; 256],
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
    pub fn process(
        &mut self,
        samples: &[f32],
        dt: f32,
        spec_cfg: &crate::audio::SpectrumAnalysisConfig,
    ) -> AudioFeatures {
        if samples.is_empty() {
            return self.tick_idle(dt, spec_cfg);
        }

        let dt = dt.clamp(1.0 / 240.0, 1.0 / 20.0);
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

        // Peak-weighted bands so a kick / bass note jumps instead of a constant RMS wall.
        let bass_raw = band_level(&self.mag_scratch, bin_hz, 50.0, 120.0, nyquist_bins)
            * sens
            * self.tuning.bass_gain;
        let low_mid_raw = band_level(&self.mag_scratch, bin_hz, 120.0, 400.0, nyquist_bins)
            * sens
            * self.tuning.low_mid_gain;
        // Kick *body* only — exclude sub rumble (<50 Hz) that basslines live in.
        let kick_body = band_level(&self.mag_scratch, bin_hz, 55.0, 95.0, nyquist_bins) * sens;
        let kick_click = band_level(&self.mag_scratch, bin_hz, 1_800.0, 5_000.0, nyquist_bins) * sens;
        // Unwindowed newest samples — Hann zeros the edges of the FFT buffer.
        let kick_attack = time_domain_attack(samples, gain);

        let band_raw = [
            gate(rms_raw, threshold),
            gate(bass_raw, threshold),
            gate(low_mid_raw, threshold),
            gate(
                band_level(&self.mag_scratch, bin_hz, 400.0, 2000.0, nyquist_bins)
                    * sens
                    * self.tuning.mid_gain,
                threshold,
            ),
            gate(
                band_level(&self.mag_scratch, bin_hz, 2000.0, 6000.0, nyquist_bins)
                    * sens
                    * self.tuning.high_mid_gain,
                threshold,
            ),
            gate(
                band_level(&self.mag_scratch, bin_hz, 6000.0, 16_000.0, nyquist_bins)
                    * sens
                    * self.tuning.high_gain,
                threshold,
            ),
        ];

        for i in 0..6 {
            // Bass / low-mid: pull out variation above the rumble floor.
            let contrast = if i == 1 || i == 2 { 0.78 } else { 0.28 };
            let relative = relative_to_floor(&mut self.band_floor[i], band_raw[i], contrast);
            let shaped = soft_ceiling(relative, ceiling);
            // Snappier envelopes on the low end so kicks read on the meters.
            let (atk, rel) = match i {
                1 => ((attack * 1.85).clamp(0.12, 0.9), (release * 1.35).clamp(0.08, 0.55)),
                2 => ((attack * 1.55).clamp(0.1, 0.85), (release * 1.2).clamp(0.07, 0.5)),
                _ => (attack, release),
            };
            self.smooth_bands[i] = smooth_dt(self.smooth_bands[i], shaped, dt, atk, rel);
        }

        let kick_rel = relative_to_floor(&mut self.kick_floor, kick_attack, 0.55);
        let kick_shaped = soft_ceiling(kick_rel, ceiling);
        self.smooth_kick = smooth_dt(self.smooth_kick, kick_shaped, dt, 0.78, 0.28);

        let processed = self
            .spectrum
            .process(
                &self.mag_scratch,
                self.sample_rate,
                self.fft_size,
                dt,
                spec_cfg,
                &self.tuning,
            )
            .clone();

        let bass_s = self.smooth_bands[1].clamp(0.0, 1.0);
        let rms_s = self.smooth_bands[0].clamp(0.0, 1.0);
        let kick_s = self.smooth_kick.clamp(0.0, 1.0);
        let beat = self.beat.process(bass_s, rms_s, kick_body, kick_click, kick_attack);

        fill_waveform(samples, gain, &mut self.waveform);

        AudioFeatures {
            rms: rms_s,
            bass: bass_s,
            low_mid: self.smooth_bands[2].clamp(0.0, 1.0),
            mid: self.smooth_bands[3].clamp(0.0, 1.0),
            high_mid: self.smooth_bands[4].clamp(0.0, 1.0),
            high: self.smooth_bands[5].clamp(0.0, 1.0),
            beat,
            kick: kick_s,
            sub: processed.sub,
            overall_energy: processed.overall_energy,
            transient_energy: processed.transient_energy,
            spectrum: processed.smoothed_bands.clone(),
            spectrum_raw: processed.bands.clone(),
            spectrum_peaks: processed.peak_hold.clone(),
            waveform: self.waveform.clone(),
            processed,
        }
    }

    pub fn tick_idle(
        &mut self,
        dt: f32,
        spec_cfg: &crate::audio::SpectrumAnalysisConfig,
    ) -> AudioFeatures {
        let dt = dt.clamp(1.0 / 240.0, 1.0 / 20.0);
        let release = self.tuning.release.clamp(0.01, 1.0);
        for b in &mut self.smooth_bands {
            *b = smooth_dt(*b, 0.0, dt, release, release);
        }
        self.smooth_kick = smooth_dt(self.smooth_kick, 0.0, dt, release, release);
        for s in &mut self.waveform {
            *s = smooth_dt(*s, 0.0, dt, release * 0.7, release * 0.7);
        }
        let processed = self.spectrum.decay(dt, spec_cfg).clone();
        let beat = self.beat.process(
            self.smooth_bands[1].clamp(0.0, 1.0),
            self.smooth_bands[0].clamp(0.0, 1.0),
            0.0,
            0.0,
            0.0,
        );
        AudioFeatures {
            rms: self.smooth_bands[0].clamp(0.0, 1.0),
            bass: self.smooth_bands[1].clamp(0.0, 1.0),
            low_mid: self.smooth_bands[2].clamp(0.0, 1.0),
            mid: self.smooth_bands[3].clamp(0.0, 1.0),
            high_mid: self.smooth_bands[4].clamp(0.0, 1.0),
            high: self.smooth_bands[5].clamp(0.0, 1.0),
            beat,
            kick: self.smooth_kick.clamp(0.0, 1.0),
            sub: processed.sub,
            overall_energy: processed.overall_energy,
            transient_energy: processed.transient_energy,
            spectrum: processed.smoothed_bands.clone(),
            spectrum_raw: processed.bands.clone(),
            spectrum_peaks: processed.peak_hold.clone(),
            waveform: self.waveform.clone(),
            processed,
        }
    }
}

/// Resample the newest chunk into a fixed-length scope buffer (−1..1), DC-removed.
fn fill_waveform(samples: &[f32], gain: f32, out: &mut [f32]) {
    if out.is_empty() {
        return;
    }
    if samples.is_empty() {
        out.fill(0.0);
        return;
    }
    let n = out.len();
    let take = samples.len().min(2048).max(n);
    let slice = &samples[samples.len() - take..];
    let last = (slice.len() - 1).max(1);
    let mut mean = 0.0_f32;
    for (i, dst) in out.iter_mut().enumerate() {
        let idx = i * last / (n - 1).max(1);
        let v = slice[idx] * gain * 2.4;
        *dst = v;
        mean += v;
    }
    mean /= n as f32;
    for dst in out.iter_mut() {
        *dst = (*dst - mean).clamp(-1.0, 1.0);
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

/// Frame-rate independent attack/release. `attack`/`release` are 60 FPS coefficients.
fn smooth_dt(prev: f32, next: f32, dt: f32, attack: f32, release: f32) -> f32 {
    let coeff = if next > prev { attack } else { release };
    let coeff = coeff.clamp(0.02, 0.98);
    let tau = -(1.0 / 60.0) / (1.0 - coeff).ln();
    let alpha = 1.0 - (-dt / tau.max(1e-4)).exp();
    prev + (next - prev) * alpha.clamp(0.0, 1.0)
}

fn time_domain_attack(samples: &[f32], gain: f32) -> f32 {
    // Newest ~10 ms vs the 10 ms before that, plus high-pass energy (derivative).
    // Bass notes / rumble have energy; kicks have a sharp rise.
    const SHORT: usize = 480; // 10 ms at 48 kHz
    if samples.len() < SHORT * 2 {
        return 0.0;
    }
    let n = samples.len();
    let now = &samples[n - SHORT..];
    let prev = &samples[n - SHORT * 2..n - SHORT];

    let rms_now = rms_of(now) * gain;
    let rms_prev = rms_of(prev) * gain;
    let flux = (rms_now - rms_prev).max(0.0);

    let mut hp_sq = 0.0_f32;
    let mut last = now[0];
    for &s in &now[1..] {
        let d = s - last;
        hp_sq += d * d;
        last = s;
    }
    let hp = (hp_sq / SHORT as f32).sqrt() * gain;
    // Rumble is loud but smooth (low ratio). Kicks are a spike.
    let ratio = hp / (rms_now + 0.02);
    let transient = ((ratio - 0.1) / 0.22).clamp(0.0, 1.6);
    (hp * 5.0 * transient + flux * 2.2 * transient).clamp(0.0, 2.0)
}

fn rms_of(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let mut sq = 0.0_f32;
    for &s in samples {
        sq += s * s;
    }
    (sq / samples.len() as f32).sqrt()
}

fn relative_to_floor(floor: &mut f32, energy: f32, contrast: f32) -> f32 {
    let e = energy.max(0.0);
    if e > *floor {
        *floor = *floor * 0.99 + e * 0.01;
    } else {
        *floor = *floor * 0.955 + e * 0.045;
    }
    let floor = (*floor).max(0.008);
    let above = (e - floor * 0.72).max(0.0);
    let relative = above / (floor * 0.5 + 0.07);
    let contrast = contrast.clamp(0.0, 1.0);
    e * (1.0 - contrast) + relative * contrast
}

fn band_level(mags: &[f32], bin_hz: f32, f_lo: f32, f_hi: f32, nyquist_bins: usize) -> f32 {
    let i0 = ((f_lo / bin_hz) as usize).clamp(1, nyquist_bins.saturating_sub(1));
    let i1 = ((f_hi / bin_hz) as usize).clamp(i0 + 1, nyquist_bins);
    let mut sum = 0.0_f32;
    let mut peak = 0.0_f32;
    let mut count = 0_u32;
    for mag in &mags[i0..i1] {
        sum += mag * mag;
        peak = peak.max(*mag);
        count += 1;
    }
    if count == 0 {
        return 0.0;
    }
    let rms = (sum / count as f32).sqrt();
    // Peak carries the hit; a little RMS keeps sustained notes visible.
    let mixed = peak * 0.7 + rms * 0.3;
    (mixed * 7.0).powf(0.88).clamp(0.0, 2.0)
}
