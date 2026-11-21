//! Shared FFT → frequency-band processor.
//!
//! Independent of rendering. One pipeline feeds the radial spectrum and
//! every other audio-reactive visual.

use super::analyzer::BandTuning;
use serde::{Deserialize, Serialize};

/// Normalized 0..1 audio features derived from a single FFT.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessedSpectrum {
    /// Pre-smoothing band magnitudes (0..1).
    pub bands: Vec<f32>,
    /// Attack/release smoothed magnitudes (0..1), plus a light transient.
    pub smoothed_bands: Vec<f32>,
    /// Decaying peak per band — for an optional LED-style cap later.
    pub peak_hold: Vec<f32>,
    /// 20–60 Hz.
    pub sub: f32,
    /// 60–150 Hz.
    pub bass: f32,
    /// 150–500 Hz.
    pub low_mid: f32,
    /// 500 Hz–2 kHz.
    pub mid: f32,
    /// 2–8 kHz.
    pub high_mid: f32,
    /// 8–18 kHz.
    pub treble: f32,
    pub overall_energy: f32,
    pub transient_energy: f32,
}

impl Default for ProcessedSpectrum {
    fn default() -> Self {
        Self {
            bands: vec![0.0; 224],
            smoothed_bands: vec![0.0; 224],
            peak_hold: vec![0.0; 224],
            sub: 0.0,
            bass: 0.0,
            low_mid: 0.0,
            mid: 0.0,
            high_mid: 0.0,
            treble: 0.0,
            overall_energy: 0.0,
            transient_energy: 0.0,
        }
    }
}

/// Analysis knobs for the shared spectrum processor.
#[derive(Debug, Clone)]
pub struct SpectrumAnalysisConfig {
    pub bar_count: u32,
    pub min_frequency: f32,
    pub max_frequency: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    pub noise_floor_db: f32,
    pub ceiling_db: f32,
    pub response_exponent: f32,
    pub frequency_smoothing: f32,
    pub bass_weight: f32,
    pub mid_weight: f32,
    pub treble_weight: f32,
    pub transient_scale: f32,
    pub sensitivity: f32,
}

impl Default for SpectrumAnalysisConfig {
    fn default() -> Self {
        Self {
            bar_count: 192,
            min_frequency: 30.0,
            max_frequency: 16_000.0,
            attack_ms: 28.0,
            release_ms: 210.0,
            noise_floor_db: -66.0,
            ceiling_db: -14.0,
            response_exponent: 1.7,
            frequency_smoothing: 0.34,
            bass_weight: 0.95,
            mid_weight: 1.12,
            treble_weight: 0.88,
            transient_scale: 0.12,
            sensitivity: 1.25,
        }
    }
}

impl SpectrumAnalysisConfig {
    pub fn from_visualizer(
        bar_count: u32,
        min_frequency: f32,
        max_frequency: f32,
        attack_ms: f32,
        release_ms: f32,
        noise_floor_db: f32,
        ceiling_db: f32,
        response_exponent: f32,
        frequency_smoothing: f32,
        bass_weight: f32,
        mid_weight: f32,
        treble_weight: f32,
        transient_scale: f32,
        sensitivity: f32,
    ) -> Self {
        Self {
            bar_count: bar_count.clamp(32, 512),
            min_frequency,
            max_frequency,
            attack_ms,
            release_ms,
            noise_floor_db,
            ceiling_db,
            response_exponent,
            frequency_smoothing,
            bass_weight,
            mid_weight,
            treble_weight,
            transient_scale,
            sensitivity,
        }
    }
}

#[derive(Clone, Copy)]
struct BandDef {
    f_lo: f32,
    f_hi: f32,
    f_center: f32,
    bin_lo: usize,
    bin_hi: usize,
    attack_s: f32,
    release_s: f32,
}

struct BandMapKey {
    fft_size: usize,
    sample_rate: u32,
    bar_count: u32,
    min_frequency: u32,
    max_frequency: u32,
    attack_ms: u32,
    release_ms: u32,
}

impl BandMapKey {
    fn from_parts(
        fft_size: usize,
        sample_rate: u32,
        cfg: &SpectrumAnalysisConfig,
    ) -> Self {
        Self {
            fft_size,
            sample_rate,
            bar_count: cfg.bar_count,
            min_frequency: cfg.min_frequency.round() as u32,
            max_frequency: cfg.max_frequency.round() as u32,
            attack_ms: cfg.attack_ms.round() as u32,
            release_ms: cfg.release_ms.round() as u32,
        }
    }
}

impl PartialEq for BandMapKey {
    fn eq(&self, other: &Self) -> bool {
        self.fft_size == other.fft_size
            && self.sample_rate == other.sample_rate
            && self.bar_count == other.bar_count
            && self.min_frequency == other.min_frequency
            && self.max_frequency == other.max_frequency
            && self.attack_ms == other.attack_ms
            && self.release_ms == other.release_ms
    }
}

/// Log-spaced bands, dB mapping, perceptual weights, dt-based smoothing.
pub struct SpectrumProcessor {
    bands: Vec<BandDef>,
    map_key: Option<BandMapKey>,
    raw: Vec<f32>,
    smoothed: Vec<f32>,
    peak_hold: Vec<f32>,
    transient: Vec<f32>,
    scratch: Vec<f32>,
    output: ProcessedSpectrum,
}

impl SpectrumProcessor {
    pub fn new(bar_count: usize) -> Self {
        let n = bar_count.clamp(32, 512);
        Self {
            bands: Vec::with_capacity(n),
            map_key: None,
            raw: vec![0.0; n],
            smoothed: vec![0.0; n],
            peak_hold: vec![0.0; n],
            transient: vec![0.0; n],
            scratch: vec![0.0; n],
            output: ProcessedSpectrum::default(),
        }
    }

    pub fn output(&self) -> &ProcessedSpectrum {
        &self.output
    }

    pub fn process(
        &mut self,
        mags: &[f32],
        sample_rate: u32,
        fft_size: usize,
        dt: f32,
        cfg: &SpectrumAnalysisConfig,
        tuning: &BandTuning,
    ) -> &ProcessedSpectrum {
        self.rebuild_if_needed(mags.len(), sample_rate, fft_size, cfg);
        let n = self.bands.len();
        if n == 0 || mags.len() < 2 {
            return &self.output;
        }

        let dt = dt.clamp(1.0 / 240.0, 1.0 / 20.0);
        let bin_hz = sample_rate as f32 / fft_size as f32;
        let fft_norm = 2.0 / (fft_size as f32).sqrt();
        let sens = cfg.sensitivity.clamp(0.05, 8.0);
        let floor_db = cfg.noise_floor_db;
        let ceil_db = cfg.ceiling_db.max(floor_db + 8.0);
        let range_db = (ceil_db - floor_db).max(8.0);
        let exponent = cfg.response_exponent.clamp(0.6, 3.0);
        let transient_scale = cfg.transient_scale.clamp(0.0, 1.0);

        for i in 0..n {
            let band = self.bands[i];
            let mixed = band_magnitude(mags, bin_hz, band) * sens;
            let mag_fs = mixed * fft_norm;
            let db = 20.0 * mag_fs.max(1e-12).log10();
            let normalized = ((db - floor_db) / range_db).clamp(0.0, 1.0);
            let weighted = (normalized
                * perceptual_weight(band.f_center, cfg)
                * tuning_gain(band.f_center, tuning))
            .clamp(0.0, 1.4);
            self.raw[i] = weighted.powf(exponent).clamp(0.0, 1.0);
        }

        let blur = cfg.frequency_smoothing.clamp(0.0, 1.0);
        if blur > 0.02 {
            neighbor_blur(&self.raw, &mut self.scratch, n, blur);
            self.raw[..n].copy_from_slice(&self.scratch[..n]);
        }

        let mut rise_sum = 0.0_f32;
        let peak_tau = 0.75_f32;
        let peak_alpha = 1.0 - (-dt / peak_tau).exp();
        let trans_atk = 1.0 - (-dt / 0.012).exp();
        let trans_rel = 1.0 - (-dt / 0.08).exp();

        for i in 0..n {
            let target = self.raw[i];
            let prev = self.smoothed[i];
            let alpha = if target > prev {
                1.0 - (-dt / self.bands[i].attack_s).exp()
            } else {
                1.0 - (-dt / self.bands[i].release_s).exp()
            };
            let smooth = prev + (target - prev) * alpha.clamp(0.0, 1.0);
            self.smoothed[i] = smooth;

            let rise = (target - prev).max(0.0);
            let t_alpha = if rise > self.transient[i] {
                trans_atk
            } else {
                trans_rel
            };
            self.transient[i] += (rise - self.transient[i]) * t_alpha;
            rise_sum += self.transient[i];

            let displayed = (smooth + self.transient[i] * transient_scale).clamp(0.0, 1.0);
            self.scratch[i] = displayed;

            if displayed > self.peak_hold[i] {
                self.peak_hold[i] = displayed;
            } else {
                self.peak_hold[i] += (0.0 - self.peak_hold[i]) * peak_alpha * 0.35;
                self.peak_hold[i] = self.peak_hold[i].max(displayed);
            }
        }

        copy_vec(&mut self.output.bands, &self.raw, n);
        copy_vec(&mut self.output.smoothed_bands, &self.scratch, n);
        copy_vec(&mut self.output.peak_hold, &self.peak_hold, n);

        self.output.sub = range_energy(&self.scratch, &self.bands, 20.0, 60.0);
        self.output.bass = range_energy(&self.scratch, &self.bands, 60.0, 150.0);
        self.output.low_mid = range_energy(&self.scratch, &self.bands, 150.0, 500.0);
        self.output.mid = range_energy(&self.scratch, &self.bands, 500.0, 2_000.0);
        self.output.high_mid = range_energy(&self.scratch, &self.bands, 2_000.0, 8_000.0);
        self.output.treble = range_energy(&self.scratch, &self.bands, 8_000.0, 18_000.0);

        let mut e2 = 0.0_f32;
        for v in &self.scratch[..n] {
            e2 += *v * *v;
        }
        self.output.overall_energy = (e2 / n as f32).sqrt().clamp(0.0, 1.0);
        self.output.transient_energy = (rise_sum / n as f32).clamp(0.0, 1.0);
        &self.output
    }

    pub fn decay(&mut self, dt: f32, cfg: &SpectrumAnalysisConfig) -> &ProcessedSpectrum {
        let n = self.smoothed.len();
        if n == 0 {
            return &self.output;
        }
        let dt = dt.clamp(1.0 / 240.0, 1.0 / 20.0);
        let tau = (cfg.release_ms / 1000.0).max(0.04);
        let alpha = 1.0 - (-dt / tau).exp();
        let peak_alpha = 1.0 - (-dt / 0.55).exp();
        for i in 0..n {
            self.raw[i] *= 1.0 - alpha;
            self.smoothed[i] *= 1.0 - alpha;
            self.transient[i] *= 1.0 - alpha * 1.4;
            self.peak_hold[i] *= 1.0 - peak_alpha * 0.5;
            self.scratch[i] = self.smoothed[i];
        }
        copy_vec(&mut self.output.bands, &self.raw, n);
        copy_vec(&mut self.output.smoothed_bands, &self.scratch, n);
        copy_vec(&mut self.output.peak_hold, &self.peak_hold, n);
        self.output.sub *= 1.0 - alpha;
        self.output.bass *= 1.0 - alpha;
        self.output.low_mid *= 1.0 - alpha;
        self.output.mid *= 1.0 - alpha;
        self.output.high_mid *= 1.0 - alpha;
        self.output.treble *= 1.0 - alpha;
        self.output.overall_energy *= 1.0 - alpha;
        self.output.transient_energy *= 1.0 - alpha * 1.5;
        &self.output
    }

    fn rebuild_if_needed(
        &mut self,
        mag_bins: usize,
        sample_rate: u32,
        fft_size: usize,
        cfg: &SpectrumAnalysisConfig,
    ) {
        let key = BandMapKey::from_parts(fft_size, sample_rate, cfg);
        if self.map_key.as_ref() == Some(&key) && self.bands.len() == cfg.bar_count as usize {
            return;
        }
        let n = cfg.bar_count.clamp(32, 512) as usize;
        let sr = sample_rate.max(1) as f32;
        let nyquist = sr * 0.5;
        let f_min = cfg.min_frequency.clamp(20.0, 200.0);
        let f_max = cfg.max_frequency.clamp(f_min * 4.0, nyquist.max(f_min * 4.0));
        let ratio = (f_max / f_min).max(2.0);
        let bin_hz = sr / fft_size.max(2) as f32;
        let nyquist_bins = mag_bins.max(2);
        let attack_s = (cfg.attack_ms / 1000.0).clamp(0.008, 0.25);
        let release_s = (cfg.release_ms / 1000.0).clamp(0.04, 0.8);

        self.bands.clear();
        self.bands.reserve(n);
        for i in 0..n {
            let t0 = i as f32 / n as f32;
            let t1 = (i + 1) as f32 / n as f32;
            let f_lo = f_min * ratio.powf(t0);
            let f_hi = f_min * ratio.powf(t1);
            let f_center = (f_lo * f_hi).sqrt();
            let bin_lo = ((f_lo / bin_hz) as usize).clamp(1, nyquist_bins.saturating_sub(1));
            let bin_hi = ((f_hi / bin_hz) as usize).clamp(bin_lo + 1, nyquist_bins);
            let (atk, rel) = freq_envelopes(f_center, attack_s, release_s);
            self.bands.push(BandDef {
                f_lo,
                f_hi,
                f_center,
                bin_lo,
                bin_hi,
                attack_s: atk,
                release_s: rel,
            });
        }
        resize_keep(&mut self.raw, n);
        resize_keep(&mut self.smoothed, n);
        resize_keep(&mut self.peak_hold, n);
        resize_keep(&mut self.transient, n);
        resize_keep(&mut self.scratch, n);
        self.map_key = Some(key);
    }
}

fn copy_vec(dst: &mut Vec<f32>, src: &[f32], n: usize) {
    if dst.len() != n {
        dst.resize(n, 0.0);
    }
    dst.copy_from_slice(&src[..n]);
}

fn resize_keep(v: &mut Vec<f32>, n: usize) {
    if v.len() != n {
        v.resize(n, 0.0);
    }
}

fn band_magnitude(mags: &[f32], bin_hz: f32, band: BandDef) -> f32 {
    let i0 = band.bin_lo.min(mags.len().saturating_sub(1));
    let i1 = band.bin_hi.clamp(i0 + 1, mags.len());
    let mut sum = 0.0_f32;
    let mut peak = 0.0_f32;
    let mut count = 0_u32;
    for mag in &mags[i0..i1] {
        let m = *mag;
        sum += m * m;
        peak = peak.max(m);
        count += 1;
    }
    let center = mag_interp(mags, bin_hz, band.f_center);
    if count == 0 {
        return center;
    }
    let rms = (sum / count as f32).sqrt();
    if band.f_hi - band.f_lo > bin_hz * 1.25 {
        peak * 0.58 + rms * 0.27 + center * 0.15
    } else {
        center * 0.8 + peak * 0.2
    }
}

fn mag_interp(mags: &[f32], bin_hz: f32, freq: f32) -> f32 {
    if bin_hz <= 0.0 || mags.len() < 2 {
        return 0.0;
    }
    let x = (freq / bin_hz).clamp(1.0, (mags.len() - 1) as f32 - 0.001);
    let i = x.floor() as usize;
    let t = x - i as f32;
    let a = mags[i];
    let b = mags[(i + 1).min(mags.len() - 1)];
    a * (1.0 - t) + b * t
}

fn neighbor_blur(src: &[f32], dst: &mut [f32], n: usize, amount: f32) {
    let w = (amount * 0.22).clamp(0.0, 0.35);
    let mid = 1.0 - 2.0 * w;
    for i in 0..n {
        let l = if i == 0 { src[i] } else { src[i - 1] };
        let r = if i + 1 >= n { src[i] } else { src[i + 1] };
        dst[i] = l * w + src[i] * mid + r * w;
    }
}

fn freq_envelopes(freq: f32, attack_s: f32, release_s: f32) -> (f32, f32) {
    if freq < 60.0 {
        (attack_s * 1.15, release_s * 1.45)
    } else if freq < 150.0 {
        (attack_s * 0.9, release_s * 1.25)
    } else if freq < 500.0 {
        (attack_s, release_s)
    } else if freq < 2_000.0 {
        (attack_s * 0.82, release_s * 0.88)
    } else if freq < 8_000.0 {
        (attack_s * 0.62, release_s * 0.7)
    } else {
        (attack_s * 0.5, release_s * 0.55)
    }
}

fn perceptual_weight(freq: f32, cfg: &SpectrumAnalysisConfig) -> f32 {
    let bass = cfg.bass_weight.clamp(0.2, 2.5);
    let mid = cfg.mid_weight.clamp(0.2, 2.5);
    let treble = cfg.treble_weight.clamp(0.2, 2.5);
    if freq < 60.0 {
        bass * 0.95
    } else if freq < 150.0 {
        bass
    } else if freq < 500.0 {
        lerp(bass, mid, (freq - 150.0) / 350.0)
    } else if freq < 2_000.0 {
        mid
    } else if freq < 8_000.0 {
        lerp(mid, treble, (freq - 2_000.0) / 6_000.0) * 0.92
    } else {
        treble * 0.72
    }
}

fn tuning_gain(freq: f32, tuning: &BandTuning) -> f32 {
    if freq < 130.0 {
        (tuning.bass_gain * tuning.spectrum_bass_tilt).clamp(0.05, 2.0)
    } else if freq < 400.0 {
        tuning.low_mid_gain.clamp(0.05, 2.5)
    } else if freq < 2_000.0 {
        tuning.mid_gain.clamp(0.05, 2.5)
    } else if freq < 6_000.0 {
        tuning.high_mid_gain.clamp(0.05, 2.5)
    } else {
        tuning.high_gain.clamp(0.05, 3.0)
    }
}

fn range_energy(values: &[f32], bands: &[BandDef], lo: f32, hi: f32) -> f32 {
    let mut sum = 0.0_f32;
    let mut wsum = 0.0_f32;
    for (v, b) in values.iter().zip(bands.iter()) {
        if b.f_center >= lo && b.f_center < hi {
            let w = (b.f_hi - b.f_lo).max(1.0);
            sum += *v * w;
            wsum += w;
        }
    }
    if wsum <= 0.0 {
        0.0
    } else {
        (sum / wsum).clamp(0.0, 1.0)
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

/// Visual height scale so highs sparkle instead of dominating the ring.
pub fn frequency_height_scale(freq_hz: f32) -> f32 {
    if freq_hz < 60.0 {
        1.12
    } else if freq_hz < 150.0 {
        1.2
    } else if freq_hz < 500.0 {
        1.0
    } else if freq_hz < 2_000.0 {
        0.82
    } else if freq_hz < 8_000.0 {
        0.52
    } else {
        0.32
    }
}

pub fn band_center_hz(index: usize, count: usize, min_hz: f32, max_hz: f32) -> f32 {
    if count == 0 {
        return min_hz;
    }
    let t = (index as f32 + 0.5) / count as f32;
    let min_hz = min_hz.max(20.0);
    let max_hz = max_hz.max(min_hz * 2.0);
    min_hz * (max_hz / min_hz).powf(t)
}
