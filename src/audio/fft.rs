//! FFT wrapper around rustfft with reusable buffers.

use rustfft::{Fft, FftPlanner, num_complex::Complex};
use std::sync::Arc;

pub struct FftProcessor {
    fft: Arc<dyn Fft<f32>>,
    scratch: Vec<Complex<f32>>,
    complex: Vec<Complex<f32>>,
}

impl FftProcessor {
    pub fn new(size: usize) -> Self {
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(size);
        Self {
            scratch: vec![Complex::new(0.0, 0.0); fft.get_inplace_scratch_len().max(1)],
            complex: vec![Complex::new(0.0, 0.0); size],
            fft,
        }
    }

    pub fn size(&self) -> usize {
        self.complex.len()
    }

    /// Compute magnitude spectrum (DC..Nyquist-1) into `out` (len = size/2).
    pub fn compute_magnitudes(&mut self, time_domain: &[f32], out: &mut [f32]) {
        let n = self.complex.len();
        debug_assert_eq!(time_domain.len(), n);
        debug_assert!(out.len() >= n / 2);

        for i in 0..n {
            self.complex[i] = Complex::new(time_domain[i], 0.0);
        }

        self.fft
            .process_with_scratch(&mut self.complex, &mut self.scratch);

        let scale = 1.0 / (n as f32).sqrt();
        let half = n / 2;
        for i in 0..half {
            let c = self.complex[i];
            out[i] = c.norm() * scale;
        }
    }
}
