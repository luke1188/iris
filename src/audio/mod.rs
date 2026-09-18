//! Audio capture, FFT analysis, and beat detection.

#![allow(dead_code)]

mod analyzer;
mod beat;
mod capture;
mod fft;
mod spectrum;

pub use analyzer::{Analyzer, AudioFeatures, BandTuning};
pub use beat::BeatMode;
pub use capture::{AudioCapture, AudioDeviceInfo};
pub use spectrum::{
    band_center_hz, frequency_height_scale, ProcessedSpectrum, SpectrumAnalysisConfig,
    SpectrumProcessor,
};
