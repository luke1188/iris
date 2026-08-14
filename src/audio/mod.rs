//! Audio capture, FFT analysis, and beat detection.

#![allow(dead_code)]

mod analyzer;
mod beat;
mod capture;
mod fft;

pub use analyzer::{Analyzer, AudioFeatures, BandTuning};
pub use beat::BeatMode;
pub use capture::{AudioCapture, AudioDeviceInfo};
