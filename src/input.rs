//! Input Stage Module
//!
//! Provides DC blocking and input gain control for the signal chain entry point.
//!
//! # Overview
//!
//! The input stage is the first processing block in the guitar amp DSP chain.
//! It performs two essential functions:
//!
//! 1. **DC Blocking**: Removes any DC offset from the input signal using a
//!    high-pass filter at 10 Hz. This prevents DC buildup from propagating
//!    through the signal chain and causing issues in subsequent stages.
//!
//! 2. **Input Gain**: Provides configurable gain from 0 dB to +20 dB to
//!    match input levels from different sources (passive pickups, active
//!    pickups, line-level signals).
//!
//! # Design Philosophy
//!
//! - **Immutable parameters**: Gain is set via a dedicated method, returning
//!   the struct with modified state
//! - **Mutable state**: Only the filter's delay line changes during processing
//! - **No heap allocation**: Fixed-size struct suitable for `no_std`
//!
//! # Requirements Traceability
//!
//! - **E1.1**: DC blocking high-pass filter: 1st order IIR, fc=10Hz
//! - **E1.2**: Input gain: 0dB to +20dB, configurable
//!
//! # Example
//!
//! ```ignore
//! use crate::input::InputStage;
//!
//! // Create input stage for 48kHz sample rate
//! let mut input = InputStage::new(48000.0);
//!
//! // Set +10 dB input gain
//! input.set_gain_db(10.0);
//!
//! // Process a sample
//! let output = input.process_sample(0.5);
//!
//! // Process a buffer in-place
//! input.process_buffer(&mut audio_buffer);
//! ```

use crate::biquad::Biquad;
use crate::dsp_math::db_to_linear;

/// Input stage processor with DC blocking and gain control.
///
/// This struct combines a high-pass filter for DC blocking with a configurable
/// gain stage. The DC blocking filter is a 2nd order high-pass (biquad) with
/// Butterworth response at 10 Hz.
///
/// # Fields
///
/// - `dc_block`: Biquad high-pass filter for DC removal
/// - `gain`: Linear gain multiplier (derived from dB setting)
#[derive(Debug, Clone, Copy)]
pub struct InputStage {
    /// DC blocking high-pass filter at 10 Hz
    dc_block: Biquad,
    /// Linear gain multiplier (1.0 = 0 dB, 10.0 = +20 dB)
    gain: f32,
}

impl InputStage {
    /// DC blocking filter cutoff frequency in Hz.
    const DC_BLOCK_FREQ: f32 = 10.0;

    /// Q factor for Butterworth response (maximally flat passband).
    const BUTTERWORTH_Q: f32 = 0.707;

    /// Minimum input gain in dB.
    const MIN_GAIN_DB: f32 = 0.0;

    /// Maximum input gain in dB.
    const MAX_GAIN_DB: f32 = 20.0;

    /// Creates a new input stage.
    ///
    /// Initializes the DC blocking filter at 10 Hz with Butterworth response
    /// and sets the input gain to unity (0 dB).
    ///
    /// # Arguments
    ///
    /// * `sample_rate` - Sample rate in Hz (typically 48000.0)
    ///
    /// # Returns
    ///
    /// A new `InputStage` with DC blocking enabled and unity gain.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let input_stage = InputStage::new(48000.0);
    /// ```
    #[must_use]
    pub fn new(sample_rate: f32) -> Self {
        Self {
            dc_block: Biquad::high_pass(Self::DC_BLOCK_FREQ, Self::BUTTERWORTH_Q, sample_rate),
            gain: 1.0,
        }
    }

    /// Sets input gain in decibels.
    ///
    /// The gain value is clamped to the valid range of 0 dB to +20 dB.
    /// Values below 0 dB are clamped to 0 dB, and values above +20 dB
    /// are clamped to +20 dB.
    ///
    /// # Arguments
    ///
    /// * `gain_db` - Desired gain in decibels (clamped to 0-20 dB range)
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut input_stage = InputStage::new(48000.0);
    /// input_stage.set_gain_db(10.0);  // +10 dB gain
    /// input_stage.set_gain_db(-5.0);  // Clamped to 0 dB
    /// input_stage.set_gain_db(30.0);  // Clamped to +20 dB
    /// ```
    pub fn set_gain_db(&mut self, gain_db: f32) {
        let clamped = gain_db.clamp(Self::MIN_GAIN_DB, Self::MAX_GAIN_DB);
        self.gain = db_to_linear(clamped);
    }

    /// Returns the current gain setting in decibels.
    ///
    /// # Returns
    ///
    /// The current gain in dB (0.0 to 20.0 range).
    #[must_use]
    pub fn gain_db(&self) -> f32 {
        crate::dsp_math::linear_to_db(self.gain)
    }

    /// Returns the current linear gain multiplier.
    ///
    /// # Returns
    ///
    /// The current linear gain (1.0 to 10.0 range).
    #[must_use]
    pub fn gain_linear(&self) -> f32 {
        self.gain
    }

    /// Processes a single audio sample.
    ///
    /// Applies DC blocking followed by input gain. The DC blocking filter
    /// removes any DC offset, and the gain stage scales the signal.
    ///
    /// # Arguments
    ///
    /// * `x` - Input sample
    ///
    /// # Returns
    ///
    /// Processed output sample with DC removed and gain applied.
    ///
    /// # Performance
    ///
    /// This function is marked `#[inline]` for optimal performance in the
    /// audio processing hot path.
    #[inline]
    pub fn process_sample(&mut self, x: f32) -> f32 {
        let dc_blocked = self.dc_block.process_sample(x);
        dc_blocked * self.gain
    }

    /// Processes a buffer of audio samples in-place.
    ///
    /// This is more efficient than calling `process_sample` in a loop
    /// due to better cache locality.
    ///
    /// # Arguments
    ///
    /// * `buffer` - Mutable slice of audio samples to process in-place
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut input_stage = InputStage::new(48000.0);
    /// let mut buffer = [0.1, 0.2, 0.3, 0.4];
    /// input_stage.process_buffer(&mut buffer);
    /// ```
    pub fn process_buffer(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.process_sample(*sample);
        }
    }

    /// Resets the DC blocking filter state.
    ///
    /// Call this when switching presets, processing a new audio stream,
    /// or to clear any accumulated state in the filter.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut input_stage = InputStage::new(48000.0);
    /// // Process some audio...
    /// input_stage.reset();  // Clear filter state
    /// ```
    pub fn reset(&mut self) {
        self.dc_block.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tolerance for floating-point comparisons
    const EPSILON: f32 = 1e-5;

    /// Tolerance for gain comparisons (allows for dB/linear conversion rounding)
    const GAIN_EPSILON: f32 = 0.01;

    /// Helper function to check if two floats are approximately equal
    fn approx_eq(a: f32, b: f32, epsilon: f32) -> bool {
        (a - b).abs() < epsilon
    }

    #[test]
    fn test_new_creates_unity_gain() {
        let input_stage = InputStage::new(48000.0);
        assert!(
            approx_eq(input_stage.gain_linear(), 1.0, EPSILON),
            "Initial gain should be unity (1.0), was {}",
            input_stage.gain_linear()
        );
    }

    #[test]
    fn test_dc_offset_removal() {
        // E1.1: DC blocking high-pass filter at 10 Hz
        let mut input_stage = InputStage::new(48000.0);

        // Feed constant DC signal (1.0) for 2 seconds to allow settling
        // Time constant for 10 Hz HPF is ~16ms, so 2 seconds is plenty
        let mut output = 0.0;
        for _ in 0..96000 {
            output = input_stage.process_sample(1.0);
        }

        // After settling, DC output should be near zero
        assert!(
            output.abs() < 0.02,
            "DC output after settling was {} expected ~0",
            output
        );
    }

    #[test]
    fn test_dc_offset_removal_negative() {
        let mut input_stage = InputStage::new(48000.0);

        // Feed constant negative DC signal
        let mut output = 0.0;
        for _ in 0..96000 {
            output = input_stage.process_sample(-0.5);
        }

        // After settling, DC output should be near zero
        assert!(
            output.abs() < 0.02,
            "Negative DC output after settling was {} expected ~0",
            output
        );
    }

    #[test]
    fn test_gain_0db() {
        // E1.2: 0 dB gain (unity)
        let mut input_stage = InputStage::new(48000.0);
        input_stage.set_gain_db(0.0);

        assert!(
            approx_eq(input_stage.gain_linear(), 1.0, EPSILON),
            "0 dB should give unity gain (1.0), was {}",
            input_stage.gain_linear()
        );

        // Verify gain_db returns 0
        assert!(
            approx_eq(input_stage.gain_db(), 0.0, GAIN_EPSILON),
            "gain_db should return 0.0, was {}",
            input_stage.gain_db()
        );
    }

    #[test]
    fn test_gain_10db() {
        // E1.2: +10 dB gain
        let mut input_stage = InputStage::new(48000.0);
        input_stage.set_gain_db(10.0);

        // 10 dB = 10^(10/20) = 10^0.5 ~= 3.162
        let expected = 3.162;
        assert!(
            approx_eq(input_stage.gain_linear(), expected, GAIN_EPSILON),
            "+10 dB should give ~3.162 linear gain, was {}",
            input_stage.gain_linear()
        );
    }

    #[test]
    fn test_gain_20db() {
        // E1.2: +20 dB gain (maximum)
        let mut input_stage = InputStage::new(48000.0);
        input_stage.set_gain_db(20.0);

        // 20 dB = 10^(20/20) = 10.0
        assert!(
            approx_eq(input_stage.gain_linear(), 10.0, EPSILON),
            "+20 dB should give 10.0 linear gain, was {}",
            input_stage.gain_linear()
        );
    }

    #[test]
    fn test_gain_clamping_negative() {
        // E1.2: Negative values should clamp to 0 dB
        let mut input_stage = InputStage::new(48000.0);
        input_stage.set_gain_db(-10.0);

        assert!(
            approx_eq(input_stage.gain_linear(), 1.0, EPSILON),
            "Negative gain_db should clamp to 0 dB (unity), was {}",
            input_stage.gain_linear()
        );
    }

    #[test]
    fn test_gain_clamping_above_max() {
        // E1.2: Values above 20 dB should clamp to 20 dB
        let mut input_stage = InputStage::new(48000.0);
        input_stage.set_gain_db(30.0);

        assert!(
            approx_eq(input_stage.gain_linear(), 10.0, EPSILON),
            "gain_db > 20 should clamp to +20 dB (10.0 linear), was {}",
            input_stage.gain_linear()
        );
    }

    #[test]
    fn test_gain_clamping_extreme_negative() {
        let mut input_stage = InputStage::new(48000.0);
        input_stage.set_gain_db(-100.0);

        assert!(
            approx_eq(input_stage.gain_linear(), 1.0, EPSILON),
            "Extreme negative gain_db should clamp to unity, was {}",
            input_stage.gain_linear()
        );
    }

    #[test]
    fn test_gain_clamping_extreme_positive() {
        let mut input_stage = InputStage::new(48000.0);
        input_stage.set_gain_db(100.0);

        assert!(
            approx_eq(input_stage.gain_linear(), 10.0, EPSILON),
            "Extreme positive gain_db should clamp to 10.0, was {}",
            input_stage.gain_linear()
        );
    }

    #[test]
    fn test_signal_passes_through() {
        // Verify that AC signals pass through with appropriate gain
        let mut input_stage = InputStage::new(48000.0);

        // First, let the filter settle with zeros
        for _ in 0..1000 {
            input_stage.process_sample(0.0);
        }

        // Now process a simple impulse and verify it comes through
        let output = input_stage.process_sample(1.0);

        // With unity gain and settled filter, output should be non-zero
        // (the exact value depends on filter coefficients)
        assert!(
            output.abs() > 0.0,
            "Signal should pass through the input stage"
        );
    }

    #[test]
    fn test_gain_affects_output() {
        let mut input_stage_unity = InputStage::new(48000.0);
        let mut input_stage_boosted = InputStage::new(48000.0);

        input_stage_unity.set_gain_db(0.0);
        input_stage_boosted.set_gain_db(20.0);

        // Process same signal through both
        let input = 0.1;
        let output_unity = input_stage_unity.process_sample(input);
        let output_boosted = input_stage_boosted.process_sample(input);

        // Boosted output should be approximately 10x unity output
        let ratio = output_boosted / output_unity;
        assert!(
            approx_eq(ratio, 10.0, 0.1),
            "20 dB boost should give 10x output ratio, was {}",
            ratio
        );
    }

    #[test]
    fn test_buffer_processing() {
        let mut input_stage1 = InputStage::new(48000.0);
        let mut input_stage2 = InputStage::new(48000.0);

        let input = [0.1, -0.2, 0.3, -0.4, 0.5, -0.6, 0.7, -0.8];

        // Process sample-by-sample
        let mut output1 = input;
        for sample in output1.iter_mut() {
            *sample = input_stage1.process_sample(*sample);
        }

        // Process as buffer
        let mut output2 = input;
        input_stage2.process_buffer(&mut output2);

        // Results should match exactly
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
    fn test_reset_clears_state() {
        let mut input_stage = InputStage::new(48000.0);

        // Process some DC to build up filter state
        for _ in 0..1000 {
            input_stage.process_sample(1.0);
        }

        // Reset
        input_stage.reset();

        // After reset, the first sample through should behave like fresh filter
        let mut fresh_stage = InputStage::new(48000.0);

        let output_reset = input_stage.process_sample(1.0);
        let output_fresh = fresh_stage.process_sample(1.0);

        assert!(
            approx_eq(output_reset, output_fresh, EPSILON),
            "After reset, output should match fresh instance: {} vs {}",
            output_reset,
            output_fresh
        );
    }

    #[test]
    fn test_numerical_stability() {
        let mut input_stage = InputStage::new(48000.0);

        // Process many samples without generating NaN/Inf
        for i in 0..100000 {
            let input = if i % 2 == 0 { 0.5 } else { -0.5 };
            let output = input_stage.process_sample(input);

            assert!(
                output.is_finite(),
                "Output became non-finite at sample {}",
                i
            );
        }
    }

    #[test]
    fn test_high_frequency_passthrough() {
        // High frequencies should pass through the 10 Hz high-pass with minimal attenuation
        let mut input_stage = InputStage::new(48000.0);

        // Generate a 1kHz sine wave and measure RMS
        let freq = 1000.0;
        let sample_rate = 48000.0;
        let num_samples = 4800; // 100ms = 100 cycles at 1kHz
        let omega = 2.0 * core::f32::consts::PI * freq / sample_rate;

        // First, let filter settle with the sine
        for i in 0..num_samples {
            let input = libm::sinf(omega * i as f32);
            input_stage.process_sample(input);
        }

        // Now measure output RMS
        let mut sum_sq = 0.0;
        for i in num_samples..(num_samples * 2) {
            let input = libm::sinf(omega * i as f32);
            let output = input_stage.process_sample(input);
            sum_sq += output * output;
        }
        let rms = libm::sqrtf(sum_sq / num_samples as f32);

        // Input RMS of sine wave is 1/sqrt(2) ~= 0.707
        // Output should be very close to this (within 1%)
        let expected_rms = 0.707;
        assert!(
            approx_eq(rms, expected_rms, 0.01),
            "1kHz signal RMS should be ~0.707, was {}",
            rms
        );
    }

    #[test]
    fn test_copy_derive() {
        let input_stage = InputStage::new(48000.0);
        let _copy = input_stage; // Copy should work due to derive
        let _clone = input_stage.clone(); // Clone should also work
    }
}
