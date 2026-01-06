//! Biquad Filter Implementation
//!
//! This module provides a second-order IIR (biquad) filter implementation
//! using Direct Form II Transposed structure for optimal numerical stability.
//!
//! # Overview
//!
//! Biquad filters are the fundamental building block for audio EQ and filtering.
//! This implementation supports common filter types derived from the Audio EQ Cookbook:
//!
//! - Low-pass filter (resonant)
//! - High-pass filter (resonant)
//! - Band-pass filter
//! - Peaking EQ (parametric)
//! - Low shelf
//! - High shelf
//!
//! # Usage in Guitar Amp DSP
//!
//! This biquad implementation is used throughout the signal chain:
//! - **E1 Input Stage:** DC blocking high-pass filter
//! - **E2 Input Filter:** Resonant low-pass for pickup modeling
//! - **E4 Tone Stack:** Bass/Mid/Treble EQ sections
//! - **E5 Power Amp:** Transformer low-pass filter
//!
//! # Design Philosophy
//!
//! - **Immutable coefficients:** Filter parameters are set at construction
//! - **Mutable state:** Only the delay line state changes during processing
//! - **No heap allocation:** Fixed-size struct suitable for `no_std`
//! - **Numerical stability:** Direct Form II Transposed minimizes rounding errors
//!
//! # Example
//!
//! ```ignore
//! use crate::biquad::Biquad;
//!
//! // Create a low-pass filter at 1kHz with Q=0.707 (Butterworth)
//! let mut lpf = Biquad::low_pass(1000.0, 0.707, 48000.0);
//!
//! // Process a sample
//! let output = lpf.process_sample(input);
//!
//! // Process a buffer in-place
//! lpf.process_buffer(&mut audio_buffer);
//! ```
//!
//! # References
//!
//! - Audio EQ Cookbook by Robert Bristow-Johnson
//! - <https://www.w3.org/2011/audio/audio-eq-cookbook.html>

use libm::{cosf, powf, sinf, sqrtf};

/// Mathematical constant PI as f32.
const PI: f32 = core::f32::consts::PI;

/// Second-order IIR filter using Direct Form II Transposed structure.
///
/// The transfer function is:
///
/// ```text
///         b0 + b1*z^-1 + b2*z^-2
/// H(z) = -------------------------
///          1 + a1*z^-1 + a2*z^-2
/// ```
///
/// Direct Form II Transposed difference equations:
///
/// ```text
/// y[n] = b0*x[n] + z1
/// z1   = b1*x[n] - a1*y[n] + z2
/// z2   = b2*x[n] - a2*y[n]
/// ```
///
/// This structure provides better numerical properties than Direct Form I,
/// particularly for low-frequency filters where coefficient values approach 1.0.
#[derive(Debug, Clone, Copy)]
pub struct Biquad {
    /// Numerator coefficient b0 (feedforward)
    b0: f32,
    /// Numerator coefficient b1 (feedforward, z^-1)
    b1: f32,
    /// Numerator coefficient b2 (feedforward, z^-2)
    b2: f32,
    /// Denominator coefficient a1 (feedback, z^-1), normalized (a0 = 1)
    a1: f32,
    /// Denominator coefficient a2 (feedback, z^-2), normalized (a0 = 1)
    a2: f32,
    /// State variable z1 (delay element 1)
    z1: f32,
    /// State variable z2 (delay element 2)
    z2: f32,
}

impl Biquad {
    /// Creates a new biquad filter with unity gain passthrough (bypass).
    ///
    /// This is useful as a default or placeholder filter that passes
    /// audio unchanged.
    ///
    /// # Returns
    ///
    /// A `Biquad` configured for unity passthrough.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    /// Creates a resonant low-pass filter.
    ///
    /// The low-pass filter attenuates frequencies above the cutoff frequency.
    /// The Q parameter controls the resonance peak at the cutoff frequency.
    ///
    /// # Arguments
    ///
    /// * `fc` - Cutoff frequency in Hz. Must be less than Nyquist (sample_rate / 2).
    /// * `q` - Quality factor. Higher values create a resonant peak.
    ///   - Q = 0.707 (1/sqrt(2)) gives Butterworth (maximally flat) response
    ///   - Q > 0.707 creates a peak at cutoff
    ///   - Q < 0.707 gives a gentler rolloff
    /// * `sample_rate` - Sample rate in Hz.
    ///
    /// # Returns
    ///
    /// A `Biquad` configured as a low-pass filter.
    ///
    /// # Panics
    ///
    /// Does not panic, but returns a bypass filter if parameters are invalid.
    #[must_use]
    pub fn low_pass(fc: f32, q: f32, sample_rate: f32) -> Self {
        if !Self::validate_params(fc, q, sample_rate) {
            return Self::new();
        }

        let w0 = 2.0 * PI * fc / sample_rate;
        let cos_w0 = cosf(w0);
        let sin_w0 = sinf(w0);
        let alpha = sin_w0 / (2.0 * q);

        let b0 = (1.0 - cos_w0) / 2.0;
        let b1 = 1.0 - cos_w0;
        let b2 = (1.0 - cos_w0) / 2.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha;

        Self::from_coefficients(b0, b1, b2, a0, a1, a2)
    }

    /// Creates a resonant high-pass filter.
    ///
    /// The high-pass filter attenuates frequencies below the cutoff frequency.
    /// Commonly used for DC blocking and removing subsonic content.
    ///
    /// # Arguments
    ///
    /// * `fc` - Cutoff frequency in Hz. Must be less than Nyquist.
    /// * `q` - Quality factor. See [`low_pass`](Self::low_pass) for details.
    /// * `sample_rate` - Sample rate in Hz.
    ///
    /// # Returns
    ///
    /// A `Biquad` configured as a high-pass filter.
    #[must_use]
    pub fn high_pass(fc: f32, q: f32, sample_rate: f32) -> Self {
        if !Self::validate_params(fc, q, sample_rate) {
            return Self::new();
        }

        let w0 = 2.0 * PI * fc / sample_rate;
        let cos_w0 = cosf(w0);
        let sin_w0 = sinf(w0);
        let alpha = sin_w0 / (2.0 * q);

        let b0 = (1.0 + cos_w0) / 2.0;
        let b1 = -(1.0 + cos_w0);
        let b2 = (1.0 + cos_w0) / 2.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha;

        Self::from_coefficients(b0, b1, b2, a0, a1, a2)
    }

    /// Creates a band-pass filter with constant skirt gain.
    ///
    /// The band-pass filter passes frequencies near the center frequency
    /// and attenuates frequencies above and below. The peak gain is 0 dB.
    ///
    /// # Arguments
    ///
    /// * `fc` - Center frequency in Hz.
    /// * `q` - Quality factor. Bandwidth = fc / Q.
    /// * `sample_rate` - Sample rate in Hz.
    ///
    /// # Returns
    ///
    /// A `Biquad` configured as a band-pass filter.
    #[must_use]
    pub fn band_pass(fc: f32, q: f32, sample_rate: f32) -> Self {
        if !Self::validate_params(fc, q, sample_rate) {
            return Self::new();
        }

        let w0 = 2.0 * PI * fc / sample_rate;
        let cos_w0 = cosf(w0);
        let sin_w0 = sinf(w0);
        let alpha = sin_w0 / (2.0 * q);

        // Constant skirt gain, peak gain = Q
        let b0 = sin_w0 / 2.0;
        let b1 = 0.0;
        let b2 = -sin_w0 / 2.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha;

        Self::from_coefficients(b0, b1, b2, a0, a1, a2)
    }

    /// Creates a peaking EQ filter (parametric equalizer).
    ///
    /// The peaking filter boosts or cuts frequencies around the center frequency.
    /// This is the core building block for parametric equalizers.
    ///
    /// # Arguments
    ///
    /// * `fc` - Center frequency in Hz.
    /// * `gain_db` - Gain at center frequency in dB.
    ///   - Positive values boost
    ///   - Negative values cut
    ///   - Zero gives unity gain (passthrough)
    /// * `q` - Quality factor. Higher Q gives a narrower bandwidth.
    /// * `sample_rate` - Sample rate in Hz.
    ///
    /// # Returns
    ///
    /// A `Biquad` configured as a peaking EQ filter.
    #[must_use]
    pub fn peaking(fc: f32, gain_db: f32, q: f32, sample_rate: f32) -> Self {
        if !Self::validate_params(fc, q, sample_rate) {
            return Self::new();
        }

        let w0 = 2.0 * PI * fc / sample_rate;
        let cos_w0 = cosf(w0);
        let sin_w0 = sinf(w0);
        let a = powf(10.0, gain_db / 40.0); // sqrt(10^(dB/20))
        let alpha = sin_w0 / (2.0 * q);

        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * cos_w0;
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha / a;

        Self::from_coefficients(b0, b1, b2, a0, a1, a2)
    }

    /// Creates a low shelf filter.
    ///
    /// The low shelf filter boosts or cuts frequencies below the shelf frequency.
    /// Uses a fixed slope of 1 (6 dB/octave transition).
    ///
    /// # Arguments
    ///
    /// * `fc` - Shelf frequency in Hz (midpoint of the transition).
    /// * `gain_db` - Gain below the shelf frequency in dB.
    /// * `sample_rate` - Sample rate in Hz.
    ///
    /// # Returns
    ///
    /// A `Biquad` configured as a low shelf filter.
    #[must_use]
    pub fn low_shelf(fc: f32, gain_db: f32, sample_rate: f32) -> Self {
        // Use a fixed Q for shelving filters (slope = 1)
        const SHELF_Q: f32 = 0.707;

        if !Self::validate_params(fc, SHELF_Q, sample_rate) {
            return Self::new();
        }

        let w0 = 2.0 * PI * fc / sample_rate;
        let cos_w0 = cosf(w0);
        let sin_w0 = sinf(w0);
        let a = powf(10.0, gain_db / 40.0);
        let alpha = sin_w0 / 2.0 * sqrtf((a + 1.0 / a) * (1.0 / SHELF_Q - 1.0) + 2.0);
        let two_sqrt_a_alpha = 2.0 * sqrtf(a) * alpha;

        let b0 = a * ((a + 1.0) - (a - 1.0) * cos_w0 + two_sqrt_a_alpha);
        let b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w0);
        let b2 = a * ((a + 1.0) - (a - 1.0) * cos_w0 - two_sqrt_a_alpha);
        let a0 = (a + 1.0) + (a - 1.0) * cos_w0 + two_sqrt_a_alpha;
        let a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cos_w0);
        let a2 = (a + 1.0) + (a - 1.0) * cos_w0 - two_sqrt_a_alpha;

        Self::from_coefficients(b0, b1, b2, a0, a1, a2)
    }

    /// Creates a high shelf filter.
    ///
    /// The high shelf filter boosts or cuts frequencies above the shelf frequency.
    /// Uses a fixed slope of 1 (6 dB/octave transition).
    ///
    /// # Arguments
    ///
    /// * `fc` - Shelf frequency in Hz (midpoint of the transition).
    /// * `gain_db` - Gain above the shelf frequency in dB.
    /// * `sample_rate` - Sample rate in Hz.
    ///
    /// # Returns
    ///
    /// A `Biquad` configured as a high shelf filter.
    #[must_use]
    pub fn high_shelf(fc: f32, gain_db: f32, sample_rate: f32) -> Self {
        const SHELF_Q: f32 = 0.707;

        if !Self::validate_params(fc, SHELF_Q, sample_rate) {
            return Self::new();
        }

        let w0 = 2.0 * PI * fc / sample_rate;
        let cos_w0 = cosf(w0);
        let sin_w0 = sinf(w0);
        let a = powf(10.0, gain_db / 40.0);
        let alpha = sin_w0 / 2.0 * sqrtf((a + 1.0 / a) * (1.0 / SHELF_Q - 1.0) + 2.0);
        let two_sqrt_a_alpha = 2.0 * sqrtf(a) * alpha;

        let b0 = a * ((a + 1.0) + (a - 1.0) * cos_w0 + two_sqrt_a_alpha);
        let b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0);
        let b2 = a * ((a + 1.0) + (a - 1.0) * cos_w0 - two_sqrt_a_alpha);
        let a0 = (a + 1.0) - (a - 1.0) * cos_w0 + two_sqrt_a_alpha;
        let a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cos_w0);
        let a2 = (a + 1.0) - (a - 1.0) * cos_w0 - two_sqrt_a_alpha;

        Self::from_coefficients(b0, b1, b2, a0, a1, a2)
    }

    /// Creates a biquad from raw coefficients, normalizing by a0.
    ///
    /// This is the internal constructor used by all filter type constructors.
    ///
    /// # Arguments
    ///
    /// * `b0`, `b1`, `b2` - Numerator (feedforward) coefficients
    /// * `a0`, `a1`, `a2` - Denominator (feedback) coefficients
    ///
    /// # Returns
    ///
    /// A `Biquad` with coefficients normalized so that a0 = 1.
    fn from_coefficients(b0: f32, b1: f32, b2: f32, a0: f32, a1: f32, a2: f32) -> Self {
        // Normalize by a0 to ensure a0 = 1
        let a0_inv = 1.0 / a0;
        Self {
            b0: b0 * a0_inv,
            b1: b1 * a0_inv,
            b2: b2 * a0_inv,
            a1: a1 * a0_inv,
            a2: a2 * a0_inv,
            z1: 0.0,
            z2: 0.0,
        }
    }

    /// Validates filter parameters.
    ///
    /// # Arguments
    ///
    /// * `fc` - Cutoff/center frequency
    /// * `q` - Quality factor
    /// * `sample_rate` - Sample rate
    ///
    /// # Returns
    ///
    /// `true` if parameters are valid, `false` otherwise.
    fn validate_params(fc: f32, q: f32, sample_rate: f32) -> bool {
        // Frequency must be positive and below Nyquist
        if fc <= 0.0 || fc >= sample_rate / 2.0 {
            return false;
        }
        // Q must be positive
        if q <= 0.0 {
            return false;
        }
        // Sample rate must be positive
        if sample_rate <= 0.0 {
            return false;
        }
        true
    }

    /// Processes a single audio sample through the filter.
    ///
    /// This is the core processing function using Direct Form II Transposed.
    /// It updates the internal state and returns the filtered output.
    ///
    /// # Arguments
    ///
    /// * `x` - Input sample
    ///
    /// # Returns
    ///
    /// Filtered output sample.
    ///
    /// # Performance
    ///
    /// This function is designed for the audio hot path:
    /// - No branches
    /// - Minimal memory access
    /// - Inline candidate
    #[inline]
    pub fn process_sample(&mut self, x: f32) -> f32 {
        // Direct Form II Transposed
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }

    /// Processes a buffer of audio samples in-place.
    ///
    /// This is more efficient than calling `process_sample` in a loop
    /// due to better cache locality and potential auto-vectorization.
    ///
    /// # Arguments
    ///
    /// * `buffer` - Mutable slice of audio samples to process in-place.
    #[inline]
    pub fn process_buffer(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.process_sample(*sample);
        }
    }

    /// Resets the filter state to zero.
    ///
    /// Call this when switching presets, processing a new audio stream,
    /// or when the filter parameters have changed significantly to avoid
    /// transient artifacts.
    #[inline]
    pub fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }

    /// Returns the current filter coefficients.
    ///
    /// Useful for debugging and verifying filter design.
    ///
    /// # Returns
    ///
    /// A tuple of `(b0, b1, b2, a1, a2)`. Note that `a0` is always 1.0
    /// after normalization.
    #[must_use]
    pub const fn coefficients(&self) -> (f32, f32, f32, f32, f32) {
        (self.b0, self.b1, self.b2, self.a1, self.a2)
    }

    /// Computes the magnitude response at a given frequency.
    ///
    /// This is useful for verifying filter design and plotting frequency response.
    ///
    /// # Arguments
    ///
    /// * `freq` - Frequency in Hz to evaluate
    /// * `sample_rate` - Sample rate in Hz
    ///
    /// # Returns
    ///
    /// Magnitude response (linear scale, not dB).
    #[must_use]
    pub fn magnitude_response(&self, freq: f32, sample_rate: f32) -> f32 {
        let w = 2.0 * PI * freq / sample_rate;
        let cos_w = cosf(w);
        let cos_2w = cosf(2.0 * w);
        let sin_w = sinf(w);
        let sin_2w = sinf(2.0 * w);

        // Numerator: B(e^jw) = b0 + b1*e^(-jw) + b2*e^(-2jw)
        let num_real = self.b0 + self.b1 * cos_w + self.b2 * cos_2w;
        let num_imag = -self.b1 * sin_w - self.b2 * sin_2w;
        let num_mag_sq = num_real * num_real + num_imag * num_imag;

        // Denominator: A(e^jw) = 1 + a1*e^(-jw) + a2*e^(-2jw)
        let den_real = 1.0 + self.a1 * cos_w + self.a2 * cos_2w;
        let den_imag = -self.a1 * sin_w - self.a2 * sin_2w;
        let den_mag_sq = den_real * den_real + den_imag * den_imag;

        sqrtf(num_mag_sq / den_mag_sq)
    }

    /// Computes the magnitude response in decibels at a given frequency.
    ///
    /// # Arguments
    ///
    /// * `freq` - Frequency in Hz to evaluate
    /// * `sample_rate` - Sample rate in Hz
    ///
    /// # Returns
    ///
    /// Magnitude response in dB.
    #[must_use]
    pub fn magnitude_response_db(&self, freq: f32, sample_rate: f32) -> f32 {
        let mag = self.magnitude_response(freq, sample_rate);
        20.0 * libm::log10f(mag)
    }
}

impl Default for Biquad {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tolerance for floating-point comparisons
    const EPSILON: f32 = 1e-5;
    /// Tolerance for frequency response tests (allows for minor numerical differences)
    const RESPONSE_EPSILON: f32 = 0.01;

    /// Helper function to check if two floats are approximately equal
    fn approx_eq(a: f32, b: f32, epsilon: f32) -> bool {
        (a - b).abs() < epsilon
    }

    #[test]
    fn test_bypass_unity_gain() {
        let mut filter = Biquad::new();

        // Unity passthrough should not change the signal
        assert!(approx_eq(filter.process_sample(1.0), 1.0, EPSILON));
        assert!(approx_eq(filter.process_sample(-0.5), -0.5, EPSILON));
        assert!(approx_eq(filter.process_sample(0.0), 0.0, EPSILON));
    }

    #[test]
    fn test_bypass_coefficients() {
        let filter = Biquad::new();
        let (b0, b1, b2, a1, a2) = filter.coefficients();

        assert!(approx_eq(b0, 1.0, EPSILON));
        assert!(approx_eq(b1, 0.0, EPSILON));
        assert!(approx_eq(b2, 0.0, EPSILON));
        assert!(approx_eq(a1, 0.0, EPSILON));
        assert!(approx_eq(a2, 0.0, EPSILON));
    }

    #[test]
    fn test_lowpass_dc_gain() {
        let filter = Biquad::low_pass(1000.0, 0.707, 48000.0);

        // DC gain should be unity (0 dB)
        let dc_response = filter.magnitude_response(0.001, 48000.0);
        assert!(
            approx_eq(dc_response, 1.0, RESPONSE_EPSILON),
            "DC response was {} expected ~1.0",
            dc_response
        );
    }

    #[test]
    fn test_lowpass_cutoff_attenuation() {
        let fc = 1000.0;
        let filter = Biquad::low_pass(fc, 0.707, 48000.0);

        // At cutoff frequency, Butterworth filter has -3 dB response
        let cutoff_response = filter.magnitude_response(fc, 48000.0);
        let expected = 0.707; // 1/sqrt(2) = -3 dB

        assert!(
            approx_eq(cutoff_response, expected, RESPONSE_EPSILON),
            "Cutoff response was {} expected ~0.707",
            cutoff_response
        );
    }

    #[test]
    fn test_lowpass_high_frequency_attenuation() {
        let fc = 1000.0;
        let filter = Biquad::low_pass(fc, 0.707, 48000.0);

        // Well above cutoff should be significantly attenuated
        // At 2 octaves above cutoff (4x frequency), expect ~-24 dB
        let high_freq_response = filter.magnitude_response(4000.0, 48000.0);

        assert!(
            high_freq_response < 0.1,
            "High frequency response was {} expected < 0.1",
            high_freq_response
        );
    }

    #[test]
    fn test_highpass_dc_attenuation() {
        let filter = Biquad::high_pass(1000.0, 0.707, 48000.0);

        // DC should be completely attenuated
        let dc_response = filter.magnitude_response(0.1, 48000.0);

        assert!(
            dc_response < 0.01,
            "DC response was {} expected ~0",
            dc_response
        );
    }

    #[test]
    fn test_highpass_cutoff_response() {
        let fc = 1000.0;
        let filter = Biquad::high_pass(fc, 0.707, 48000.0);

        // At cutoff frequency, Butterworth filter has -3 dB response
        let cutoff_response = filter.magnitude_response(fc, 48000.0);
        let expected = 0.707;

        assert!(
            approx_eq(cutoff_response, expected, RESPONSE_EPSILON),
            "Cutoff response was {} expected ~0.707",
            cutoff_response
        );
    }

    #[test]
    fn test_highpass_high_frequency_passthrough() {
        let fc = 1000.0;
        let filter = Biquad::high_pass(fc, 0.707, 48000.0);

        // Well above cutoff should pass through
        let high_freq_response = filter.magnitude_response(10000.0, 48000.0);

        assert!(
            approx_eq(high_freq_response, 1.0, RESPONSE_EPSILON),
            "High frequency response was {} expected ~1.0",
            high_freq_response
        );
    }

    #[test]
    fn test_bandpass_center_frequency() {
        let fc = 2000.0;
        let q = 2.0;
        let filter = Biquad::band_pass(fc, q, 48000.0);

        // At center frequency, response should be maximum
        let center_response = filter.magnitude_response(fc, 48000.0);

        // Should have significant gain at center (depends on Q)
        assert!(
            center_response > 0.9,
            "Center response was {} expected > 0.9",
            center_response
        );
    }

    #[test]
    fn test_bandpass_edge_attenuation() {
        let fc = 2000.0;
        let q = 2.0;
        let filter = Biquad::band_pass(fc, q, 48000.0);

        // DC and Nyquist should be attenuated
        let dc_response = filter.magnitude_response(10.0, 48000.0);
        let nyquist_response = filter.magnitude_response(20000.0, 48000.0);

        assert!(
            dc_response < 0.1,
            "DC response was {} expected < 0.1",
            dc_response
        );
        assert!(
            nyquist_response < 0.2,
            "Nyquist response was {} expected < 0.2",
            nyquist_response
        );
    }

    #[test]
    fn test_peaking_boost() {
        let fc = 1000.0;
        let gain_db = 6.0;
        let q = 1.0;
        let filter = Biquad::peaking(fc, gain_db, q, 48000.0);

        // At center frequency, should have approximately 6 dB boost
        let center_response_db = filter.magnitude_response_db(fc, 48000.0);

        assert!(
            approx_eq(center_response_db, gain_db, 0.5),
            "Center response was {} dB expected ~{} dB",
            center_response_db,
            gain_db
        );

        // DC and high frequencies should be unity
        let dc_response = filter.magnitude_response(10.0, 48000.0);
        let high_response = filter.magnitude_response(15000.0, 48000.0);

        assert!(
            approx_eq(dc_response, 1.0, RESPONSE_EPSILON),
            "DC response was {} expected ~1.0",
            dc_response
        );
        assert!(
            approx_eq(high_response, 1.0, RESPONSE_EPSILON),
            "High frequency response was {} expected ~1.0",
            high_response
        );
    }

    #[test]
    fn test_peaking_cut() {
        let fc = 1000.0;
        let gain_db = -6.0;
        let q = 1.0;
        let filter = Biquad::peaking(fc, gain_db, q, 48000.0);

        // At center frequency, should have approximately -6 dB cut
        let center_response_db = filter.magnitude_response_db(fc, 48000.0);

        assert!(
            approx_eq(center_response_db, gain_db, 0.5),
            "Center response was {} dB expected ~{} dB",
            center_response_db,
            gain_db
        );
    }

    #[test]
    fn test_low_shelf_boost() {
        let fc = 500.0;
        let gain_db = 6.0;
        let filter = Biquad::low_shelf(fc, gain_db, 48000.0);

        // Below shelf frequency, should have boost
        let low_response_db = filter.magnitude_response_db(50.0, 48000.0);

        assert!(
            approx_eq(low_response_db, gain_db, 1.0),
            "Low frequency response was {} dB expected ~{} dB",
            low_response_db,
            gain_db
        );

        // Above shelf frequency, should be unity
        let high_response = filter.magnitude_response(5000.0, 48000.0);

        assert!(
            approx_eq(high_response, 1.0, RESPONSE_EPSILON),
            "High frequency response was {} expected ~1.0",
            high_response
        );
    }

    #[test]
    fn test_high_shelf_boost() {
        let fc = 2000.0;
        let gain_db = 6.0;
        let filter = Biquad::high_shelf(fc, gain_db, 48000.0);

        // Above shelf frequency, should have boost
        let high_response_db = filter.magnitude_response_db(10000.0, 48000.0);

        assert!(
            approx_eq(high_response_db, gain_db, 1.0),
            "High frequency response was {} dB expected ~{} dB",
            high_response_db,
            gain_db
        );

        // Below shelf frequency, should be unity
        let low_response = filter.magnitude_response(200.0, 48000.0);

        assert!(
            approx_eq(low_response, 1.0, RESPONSE_EPSILON),
            "Low frequency response was {} expected ~1.0",
            low_response
        );
    }

    #[test]
    fn test_reset_clears_state() {
        let mut filter = Biquad::low_pass(1000.0, 0.707, 48000.0);

        // Process some samples to build up state
        for _ in 0..100 {
            filter.process_sample(1.0);
        }

        // Reset should clear state
        filter.reset();

        // After reset, processing should start fresh
        // (first output should be close to b0 * input)
        let (b0, _, _, _, _) = filter.coefficients();
        let first_output = filter.process_sample(1.0);

        assert!(
            approx_eq(first_output, b0, EPSILON),
            "First output after reset was {} expected ~{}",
            first_output,
            b0
        );
    }

    #[test]
    fn test_buffer_processing() {
        let mut filter1 = Biquad::low_pass(1000.0, 0.707, 48000.0);
        let mut filter2 = Biquad::low_pass(1000.0, 0.707, 48000.0);

        let input = [0.5, -0.3, 0.8, -0.1, 0.0, 0.2, -0.4, 0.6];

        // Process sample-by-sample
        let mut output1 = input;
        for sample in output1.iter_mut() {
            *sample = filter1.process_sample(*sample);
        }

        // Process as buffer
        let mut output2 = input;
        filter2.process_buffer(&mut output2);

        // Results should match
        for (a, b) in output1.iter().zip(output2.iter()) {
            assert!(
                approx_eq(*a, *b, EPSILON),
                "Buffer processing mismatch: {} vs {}",
                a,
                b
            );
        }
    }

    #[test]
    fn test_invalid_frequency_returns_bypass() {
        // Frequency at or above Nyquist
        let filter = Biquad::low_pass(25000.0, 0.707, 48000.0);
        let (b0, b1, b2, a1, a2) = filter.coefficients();

        assert!(approx_eq(b0, 1.0, EPSILON));
        assert!(approx_eq(b1, 0.0, EPSILON));
        assert!(approx_eq(b2, 0.0, EPSILON));
        assert!(approx_eq(a1, 0.0, EPSILON));
        assert!(approx_eq(a2, 0.0, EPSILON));

        // Negative frequency
        let filter2 = Biquad::low_pass(-100.0, 0.707, 48000.0);
        let (b0, _, _, _, _) = filter2.coefficients();
        assert!(approx_eq(b0, 1.0, EPSILON));
    }

    #[test]
    fn test_invalid_q_returns_bypass() {
        let filter = Biquad::low_pass(1000.0, 0.0, 48000.0);
        let (b0, b1, b2, a1, a2) = filter.coefficients();

        assert!(approx_eq(b0, 1.0, EPSILON));
        assert!(approx_eq(b1, 0.0, EPSILON));
        assert!(approx_eq(b2, 0.0, EPSILON));
        assert!(approx_eq(a1, 0.0, EPSILON));
        assert!(approx_eq(a2, 0.0, EPSILON));
    }

    #[test]
    fn test_dc_blocking_high_pass() {
        // Test case for E1 Input Stage: DC blocking at 10 Hz
        let mut filter = Biquad::high_pass(10.0, 0.707, 48000.0);

        // Process a DC signal (constant 1.0)
        // For a 10 Hz HPF, the time constant is approximately 1/(2*pi*10) = 16ms
        // We need several time constants to settle fully
        let mut output = 0.0;
        for _ in 0..96000 {
            // 2 seconds for adequate settling
            output = filter.process_sample(1.0);
        }

        // After settling, DC should be nearly zero
        // A 10 Hz HPF with Q=0.707 will asymptotically approach zero
        // Allow slightly looser tolerance for low-frequency numerical precision
        assert!(
            output.abs() < 0.02,
            "DC output after settling was {} expected ~0",
            output
        );
    }

    #[test]
    fn test_pickup_resonance_filter() {
        // Test case for E2 Input Filter: Resonant low-pass at 3.5 kHz
        let fc = 3500.0;
        let q = 1.5;
        let filter = Biquad::low_pass(fc, q, 48000.0);

        // Should have a peak near the cutoff due to Q > 0.707
        let cutoff_response = filter.magnitude_response(fc, 48000.0);

        // With Q = 1.5, expect a small peak
        assert!(
            cutoff_response > 0.9,
            "Resonant peak was {} expected > 0.9",
            cutoff_response
        );
    }

    #[test]
    fn test_transformer_lowpass() {
        // Test case for E5 Power Amp: Transformer LPF at 6 kHz
        let fc = 6000.0;
        let filter = Biquad::low_pass(fc, 0.707, 48000.0);

        // Should attenuate above 6 kHz
        let response_10k = filter.magnitude_response(10000.0, 48000.0);

        assert!(
            response_10k < 0.5,
            "10 kHz response was {} expected < 0.5",
            response_10k
        );
    }

    #[test]
    fn test_coefficient_symmetry() {
        // For LP/HP filters with same fc and Q, certain coefficient relationships hold
        let lp = Biquad::low_pass(1000.0, 0.707, 48000.0);
        let hp = Biquad::high_pass(1000.0, 0.707, 48000.0);

        let (lp_b0, lp_b1, lp_b2, lp_a1, lp_a2) = lp.coefficients();
        let (hp_b0, hp_b1, hp_b2, hp_a1, hp_a2) = hp.coefficients();

        // Denominator coefficients should be identical
        assert!(
            approx_eq(lp_a1, hp_a1, EPSILON),
            "a1 mismatch: LP {} vs HP {}",
            lp_a1,
            hp_a1
        );
        assert!(
            approx_eq(lp_a2, hp_a2, EPSILON),
            "a2 mismatch: LP {} vs HP {}",
            lp_a2,
            hp_a2
        );

        // LP: b0 = b2, HP: b0 = b2 (both symmetric)
        assert!(
            approx_eq(lp_b0, lp_b2, EPSILON),
            "LP b0/b2 not symmetric: {} vs {}",
            lp_b0,
            lp_b2
        );
        assert!(
            approx_eq(hp_b0, hp_b2, EPSILON),
            "HP b0/b2 not symmetric: {} vs {}",
            hp_b0,
            hp_b2
        );

        // HP b1 should be negative of sum of HP b0 + b2
        // (i.e., HP b1 = -(1 + cos_w0) while HP b0 = b2 = (1 + cos_w0)/2)
        assert!(
            approx_eq(hp_b1, -2.0 * hp_b0, EPSILON),
            "HP b1 relationship: {} vs {}",
            hp_b1,
            -2.0 * hp_b0
        );

        // LP b1 should be twice b0
        assert!(
            approx_eq(lp_b1, 2.0 * lp_b0, EPSILON),
            "LP b1 relationship: {} vs {}",
            lp_b1,
            2.0 * lp_b0
        );
    }

    #[test]
    fn test_numerical_stability_low_frequency() {
        // Very low frequency filter (potential numerical issues)
        let mut filter = Biquad::high_pass(5.0, 0.707, 48000.0);

        // Process many samples without NaN/Inf
        for i in 0..100000 {
            let input = if i % 2 == 0 { 1.0 } else { -1.0 };
            let output = filter.process_sample(input);

            assert!(output.is_finite(), "Output became non-finite at sample {}", i);
        }
    }

    #[test]
    fn test_default_is_bypass() {
        let filter = Biquad::default();
        let bypass = Biquad::new();

        let (d_b0, d_b1, d_b2, d_a1, d_a2) = filter.coefficients();
        let (b_b0, b_b1, b_b2, b_a1, b_a2) = bypass.coefficients();

        assert!(approx_eq(d_b0, b_b0, EPSILON));
        assert!(approx_eq(d_b1, b_b1, EPSILON));
        assert!(approx_eq(d_b2, b_b2, EPSILON));
        assert!(approx_eq(d_a1, b_a1, EPSILON));
        assert!(approx_eq(d_a2, b_a2, EPSILON));
    }
}
