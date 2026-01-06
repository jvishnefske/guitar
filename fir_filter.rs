//! FIR (Finite Impulse Response) Filter Implementation
//!
//! This module provides a safe, verifiable FIR filter with:
//! - Compile-time maximum tap count validation
//! - Runtime coefficient updates (thread-safe)
//! - SIMD-friendly memory layout
//! - No heap allocation (uses static buffers)

use core::sync::atomic::{AtomicBool, Ordering};
use heapless::Vec;
use micromath::F32Ext;

/// Maximum number of FIR taps supported
/// Keep power of 2 for efficient modulo operations
pub const MAX_TAPS: usize = 128;

/// Sample type alias for clarity
pub type Sample = i16;
pub type SampleFloat = f32;

/// Predefined filter types for common audio processing
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum FilterPreset {
    /// Flat response (passthrough)
    Bypass,
    /// Low-pass filter with configurable cutoff
    LowPass { cutoff_hz: f32 },
    /// High-pass filter with configurable cutoff
    HighPass { cutoff_hz: f32 },
    /// Band-pass filter
    BandPass { low_hz: f32, high_hz: f32 },
    /// Parametric EQ band
    Parametric { center_hz: f32, gain_db: f32, q: f32 },
    /// Custom coefficients from phone app
    Custom,
}

impl Default for FilterPreset {
    fn default() -> Self {
        Self::Bypass
    }
}

/// Filter parameters sent from phone app via BLE
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FilterParams {
    pub preset: FilterPreset,
    pub sample_rate: u32,
    /// Custom coefficients (only used when preset == Custom)
    pub custom_coeffs: Vec<f32, MAX_TAPS>,
}

impl Default for FilterParams {
    fn default() -> Self {
        Self {
            preset: FilterPreset::Bypass,
            sample_rate: 48000,
            custom_coeffs: Vec::new(),
        }
    }
}

/// FIR Filter with circular buffer implementation
///
/// # Safety Invariants
/// - `write_idx` is always < MAX_TAPS
/// - `coeffs` length matches `delay_line` active region
/// - All operations are constant-time for real-time safety
pub struct FirFilter {
    /// Delay line (circular buffer)
    delay_line: [SampleFloat; MAX_TAPS],
    /// Filter coefficients (impulse response)
    coefficients: [SampleFloat; MAX_TAPS],
    /// Number of active taps
    num_taps: usize,
    /// Current write position in circular buffer
    write_idx: usize,
    /// Flag indicating coefficients are being updated
    updating: AtomicBool,
    /// Current sample rate
    sample_rate: u32,
}

impl FirFilter {
    /// Create a new bypass filter (unity gain, no filtering)
    pub const fn new() -> Self {
        let mut coeffs = [0.0; MAX_TAPS];
        coeffs[0] = 1.0; // Unity impulse response
        
        Self {
            delay_line: [0.0; MAX_TAPS],
            coefficients: coeffs,
            num_taps: 1,
            write_idx: 0,
            updating: AtomicBool::new(false),
            sample_rate: 48000,
        }
    }

    /// Process a single sample through the filter
    ///
    /// This is the hot path - optimized for minimal branching
    #[inline]
    pub fn process_sample(&mut self, input: Sample) -> Sample {
        // Skip processing during coefficient update
        if self.updating.load(Ordering::Relaxed) {
            return input;
        }

        let input_f = input as SampleFloat / 32768.0;
        
        // Write input to delay line
        self.delay_line[self.write_idx] = input_f;
        
        // Compute convolution
        let mut output: SampleFloat = 0.0;
        let mut idx = self.write_idx;
        
        for i in 0..self.num_taps {
            output += self.delay_line[idx] * self.coefficients[i];
            // Decrement with wrap (circular buffer)
            idx = if idx == 0 { MAX_TAPS - 1 } else { idx - 1 };
        }
        
        // Advance write pointer
        self.write_idx = (self.write_idx + 1) % MAX_TAPS;
        
        // Convert back to i16 with saturation
        let output_scaled = output * 32767.0;
        output_scaled.clamp(-32768.0, 32767.0) as Sample
    }

    /// Process a buffer of samples in-place
    pub fn process_buffer(&mut self, buffer: &mut [Sample]) {
        for sample in buffer.iter_mut() {
            *sample = self.process_sample(*sample);
        }
    }

    /// Process stereo interleaved buffer (L, R, L, R, ...)
    pub fn process_stereo(&mut self, buffer: &mut [Sample]) {
        // Process left and right channels identically
        // For true stereo, you'd want two filter instances
        for sample in buffer.iter_mut() {
            *sample = self.process_sample(*sample);
        }
    }

    /// Update filter coefficients from parameters
    ///
    /// This is called from the BLE task when phone sends new settings
    pub fn update_params(&mut self, params: &FilterParams) {
        // Signal that we're updating (audio will bypass during update)
        self.updating.store(true, Ordering::SeqCst);
        
        self.sample_rate = params.sample_rate;
        
        match params.preset {
            FilterPreset::Bypass => {
                self.set_bypass();
            }
            FilterPreset::LowPass { cutoff_hz } => {
                self.design_lowpass(cutoff_hz);
            }
            FilterPreset::HighPass { cutoff_hz } => {
                self.design_highpass(cutoff_hz);
            }
            FilterPreset::BandPass { low_hz, high_hz } => {
                self.design_bandpass(low_hz, high_hz);
            }
            FilterPreset::Parametric { center_hz, gain_db, q } => {
                self.design_parametric(center_hz, gain_db, q);
            }
            FilterPreset::Custom => {
                if !params.custom_coeffs.is_empty() {
                    self.set_coefficients(&params.custom_coeffs);
                }
            }
        }
        
        // Clear delay line to prevent artifacts
        self.delay_line.fill(0.0);
        self.write_idx = 0;
        
        self.updating.store(false, Ordering::SeqCst);
    }

    /// Set filter to bypass mode (unity gain)
    fn set_bypass(&mut self) {
        self.coefficients.fill(0.0);
        self.coefficients[0] = 1.0;
        self.num_taps = 1;
    }

    /// Set custom coefficients
    fn set_coefficients(&mut self, coeffs: &[f32]) {
        let num = coeffs.len().min(MAX_TAPS);
        self.coefficients[..num].copy_from_slice(&coeffs[..num]);
        self.coefficients[num..].fill(0.0);
        self.num_taps = num;
    }

    /// Design a low-pass FIR filter using windowed sinc method
    ///
    /// Uses Hamming window for good sidelobe suppression
    fn design_lowpass(&mut self, cutoff_hz: f32) {
        const NUM_TAPS: usize = 63; // Odd for symmetric
        let fc = cutoff_hz / self.sample_rate as f32;
        
        self.num_taps = NUM_TAPS;
        let m = (NUM_TAPS - 1) as f32;
        
        for i in 0..NUM_TAPS {
            let n = i as f32;
            let x = n - m / 2.0;
            
            // Sinc function
            let sinc = if x.abs() < 1e-6 {
                2.0 * core::f32::consts::PI * fc
            } else {
                (2.0 * core::f32::consts::PI * fc * x).sin() / x
            };
            
            // Hamming window
            let window = 0.54 - 0.46 * (2.0 * core::f32::consts::PI * n / m).cos();
            
            self.coefficients[i] = sinc * window;
        }
        
        // Normalize for unity gain at DC
        let sum: f32 = self.coefficients[..NUM_TAPS].iter().sum();
        if sum.abs() > 1e-6 {
            for c in &mut self.coefficients[..NUM_TAPS] {
                *c /= sum;
            }
        }
    }

    /// Design a high-pass filter (spectral inversion of low-pass)
    fn design_highpass(&mut self, cutoff_hz: f32) {
        // Design lowpass first
        self.design_lowpass(cutoff_hz);
        
        // Spectral inversion
        for c in &mut self.coefficients[..self.num_taps] {
            *c = -*c;
        }
        
        // Add 1 to center tap for high-pass
        let center = self.num_taps / 2;
        self.coefficients[center] += 1.0;
    }

    /// Design a band-pass filter
    fn design_bandpass(&mut self, low_hz: f32, high_hz: f32) {
        const NUM_TAPS: usize = 63;
        let fc_low = low_hz / self.sample_rate as f32;
        let fc_high = high_hz / self.sample_rate as f32;
        
        self.num_taps = NUM_TAPS;
        let m = (NUM_TAPS - 1) as f32;
        
        for i in 0..NUM_TAPS {
            let n = i as f32;
            let x = n - m / 2.0;
            
            // Bandpass = highpass(fc_low) convolved with lowpass(fc_high)
            // Simplified: difference of two lowpass filters
            let sinc_high = if x.abs() < 1e-6 {
                2.0 * core::f32::consts::PI * fc_high
            } else {
                (2.0 * core::f32::consts::PI * fc_high * x).sin() / x
            };
            
            let sinc_low = if x.abs() < 1e-6 {
                2.0 * core::f32::consts::PI * fc_low
            } else {
                (2.0 * core::f32::consts::PI * fc_low * x).sin() / x
            };
            
            let window = 0.54 - 0.46 * (2.0 * core::f32::consts::PI * n / m).cos();
            self.coefficients[i] = (sinc_high - sinc_low) * window;
        }
        
        // Normalize for unity gain at center frequency
        let sum: f32 = self.coefficients[..NUM_TAPS]
            .iter()
            .map(|c| c.abs())
            .sum();
        if sum > 1e-6 {
            let scale = 2.0 / sum; // Approximate normalization
            for c in &mut self.coefficients[..NUM_TAPS] {
                *c *= scale;
            }
        }
    }

    /// Design a parametric EQ filter
    ///
    /// Note: True parametric EQ is IIR, but we approximate with FIR
    /// for phase linearity and stability
    fn design_parametric(&mut self, center_hz: f32, gain_db: f32, q: f32) {
        // Bandwidth from Q
        let bw = center_hz / q;
        let low = (center_hz - bw / 2.0).max(20.0);
        let high = (center_hz + bw / 2.0).min(self.sample_rate as f32 / 2.0 - 100.0);
        
        // Start with bandpass
        self.design_bandpass(low, high);
        
        // Apply gain and mix with bypass
        let gain_linear = 10.0_f32.powf(gain_db / 20.0);
        let mix = (gain_linear - 1.0).abs() / gain_linear.max(1.0);
        
        // Blend filtered signal with bypass
        let center = self.num_taps / 2;
        for (i, c) in self.coefficients[..self.num_taps].iter_mut().enumerate() {
            if i == center {
                *c = (1.0 - mix) + *c * mix * gain_linear;
            } else {
                *c *= mix * gain_linear;
            }
        }
    }

    /// Get current filter info for debugging/display
    pub fn get_info(&self) -> FilterInfo {
        FilterInfo {
            num_taps: self.num_taps,
            sample_rate: self.sample_rate,
            is_updating: self.updating.load(Ordering::Relaxed),
        }
    }
}

impl Default for FirFilter {
    fn default() -> Self {
        Self::new()
    }
}

/// Filter status information
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FilterInfo {
    pub num_taps: usize,
    pub sample_rate: u32,
    pub is_updating: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bypass_unity_gain() {
        let mut filter = FirFilter::new();
        
        // Input should equal output for bypass
        assert_eq!(filter.process_sample(1000), 1000);
        assert_eq!(filter.process_sample(-1000), -1000);
        assert_eq!(filter.process_sample(0), 0);
    }

    #[test]
    fn test_lowpass_design() {
        let mut filter = FirFilter::new();
        let params = FilterParams {
            preset: FilterPreset::LowPass { cutoff_hz: 1000.0 },
            sample_rate: 48000,
            ..Default::default()
        };
        
        filter.update_params(&params);
        
        // Should have multiple taps now
        assert!(filter.num_taps > 1);
        
        // DC gain should be approximately 1
        let sum: f32 = filter.coefficients[..filter.num_taps].iter().sum();
        assert!((sum - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_coefficient_update_safety() {
        let mut filter = FirFilter::new();
        
        // Verify atomic flag behavior
        assert!(!filter.updating.load(Ordering::Relaxed));
        
        filter.updating.store(true, Ordering::SeqCst);
        // During update, should pass through unchanged
        assert_eq!(filter.process_sample(1234), 1234);
        
        filter.updating.store(false, Ordering::SeqCst);
    }
}
